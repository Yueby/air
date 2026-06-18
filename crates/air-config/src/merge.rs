//! mihomo 运行配置合并流水线。
//!
//! 合并层只处理“把多个已解析配置片段拼成最终 YAML”的本地确定性逻辑：订阅下载、
//! GUI 表单编辑和 mihomo 进程启动分别由其他模块负责。这里的输出可以先用于预览，
//! 只有调用写入函数并且没有阻断诊断时，才会写入 mihomo 工作目录。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use air_error::ConfigError;

use super::model::{
    MihomoConfigDocument, ProxyGroup, ProxyNode, ProxyProvider, RuleLine, RuleProvider,
};
use super::{ConfigDiagnostic, ConfigDiagnosticSeverity};

const BUILTIN_POLICIES: &[&str] = &[
    "DIRECT",
    "REJECT",
    "REJECT-DROP",
    "PASS",
    "COMPATIBLE",
    "GLOBAL",
];

/// 一次配置合并的完整输入。
///
/// `profile` 是当前 profile 的强类型配置；`subscriptions` 是订阅缓存中已经成功解析的配置；
/// `overrides` 表示 GUI 尚未写回 profile 的显式覆盖；`runtime` 决定最终 YAML 落到哪个工作目录。
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigMergeInput {
    pub profile_id: Option<String>,
    pub profile: MihomoConfigDocument,
    pub subscriptions: Vec<SubscriptionMergeInput>,
    pub overrides: ConfigMergeOverrides,
    pub runtime: ConfigMergeRuntimePaths,
}

impl ConfigMergeInput {
    pub fn new(profile: MihomoConfigDocument, runtime: ConfigMergeRuntimePaths) -> Self {
        Self {
            profile_id: None,
            profile,
            subscriptions: Vec::new(),
            overrides: ConfigMergeOverrides::default(),
            runtime,
        }
    }
}

/// 单个订阅缓存参与合并时的输入。
///
/// 订阅更新流水线负责下载和解析；合并层只接收已经解析出的 `MihomoConfigDocument`，并按
/// `enabled` 决定是否将其中的 proxies、providers、groups 和 rules 拼入最终文档。
#[derive(Clone, Debug, PartialEq)]
pub struct SubscriptionMergeInput {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub document: MihomoConfigDocument,
}

impl SubscriptionMergeInput {
    pub fn enabled(
        id: impl Into<String>,
        display_name: impl Into<String>,
        document: MihomoConfigDocument,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            enabled: true,
            document,
        }
    }
}

/// GUI 覆盖项。
///
/// 覆盖项在订阅之后应用，表示用户在界面上明确做出的修改。节点、组和 provider 同名时采用
/// “覆盖已有条目”的策略；规则默认追加，也可以切换为替换主规则链。
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigMergeOverrides {
    pub proxies: Vec<ProxyNode>,
    pub proxy_groups: Vec<ProxyGroup>,
    pub proxy_providers: BTreeMap<String, ProxyProvider>,
    pub rule_providers: BTreeMap<String, RuleProvider>,
    pub rules: Vec<RuleLine>,
    pub sub_rules: BTreeMap<String, Vec<RuleLine>>,
    pub rule_mode: OverrideRuleMode,
}

impl Default for ConfigMergeOverrides {
    fn default() -> Self {
        Self {
            proxies: Vec::new(),
            proxy_groups: Vec::new(),
            proxy_providers: BTreeMap::new(),
            rule_providers: BTreeMap::new(),
            rules: Vec::new(),
            sub_rules: BTreeMap::new(),
            rule_mode: OverrideRuleMode::Append,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverrideRuleMode {
    #[default]
    Append,
    Replace,
}

/// mihomo 工作目录路径。
///
/// `work_dir` 由运行态装配层传入；合并层只拼出最终配置文件路径，不推断平台目录。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigMergeRuntimePaths {
    pub work_dir: PathBuf,
    pub config_file_name: String,
}

impl ConfigMergeRuntimePaths {
    pub fn new(work_dir: impl Into<PathBuf>) -> Self {
        Self {
            work_dir: work_dir.into(),
            config_file_name: "config.yaml".to_string(),
        }
    }

    pub fn final_config_path(&self) -> PathBuf {
        self.work_dir.join(&self.config_file_name)
    }
}

/// 预览结果同时包含最终文档、YAML 文本、变更摘要和诊断。
///
/// 预览允许返回 error 诊断，供 UI 展示；真正写入时会把 error 诊断视为阻断条件。
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigMergePreview {
    pub document: MihomoConfigDocument,
    pub yaml: String,
    pub summary: ConfigMergeSummary,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl ConfigMergePreview {
    pub fn has_errors(&self) -> bool {
        has_error_diagnostics(&self.diagnostics)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ConfigMergeSummary {
    pub output_path: PathBuf,
    pub profile_proxy_count: usize,
    pub subscription_proxy_count: usize,
    pub final_proxy_count: usize,
    pub final_group_count: usize,
    pub final_rule_count: usize,
    pub changes: Vec<ConfigMergeChange>,
}

impl Default for ConfigMergeSummary {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            profile_proxy_count: 0,
            subscription_proxy_count: 0,
            final_proxy_count: 0,
            final_group_count: 0,
            final_rule_count: 0,
            changes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigMergeChange {
    pub stage: ConfigMergeStage,
    pub path: String,
    pub kind: ConfigMergeChangeKind,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigMergeStage {
    Profile,
    Subscription,
    Override,
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigMergeChangeKind {
    Added,
    Renamed,
    Replaced,
    Skipped,
    Diagnosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigMergeWriteReport {
    pub path: PathBuf,
    pub bytes: usize,
    pub summary: ConfigMergeSummary,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigMergePipeline;

impl ConfigMergePipeline {
    pub fn new() -> Self {
        Self
    }

    /// 第一阶段：从当前 profile 克隆基础文档。
    ///
    /// 输入是当前 profile；输出是后续阶段可修改的内存文档。此阶段不做引用重写，
    /// 只记录 profile 中已有节点数量，方便最终摘要解释订阅增量。
    pub fn preview(&self, input: &ConfigMergeInput) -> Result<ConfigMergePreview, ConfigError> {
        let mut context = MergeContext::new(input);
        context.merge_subscriptions();
        context.apply_overrides();
        context.finish()
    }

    /// 最终阶段：把合并后的 YAML 原子写入 mihomo 工作目录。
    ///
    /// 输入是完整合并输入；输出是写入路径和字节数。当前主路径已由 app/core 调用
    /// mihomo `-t -f` 做最终配置语义校验；这个历史流水线只保留自身合并阶段产生的
    /// 阻断诊断，避免用 Air 内置规则替代 mihomo 内核判断。
    pub fn write_final_yaml(
        &self,
        input: &ConfigMergeInput,
    ) -> Result<ConfigMergeWriteReport, ConfigError> {
        let preview = self.preview(input)?;
        if preview.has_errors() {
            return Err(ConfigError::Validation(format!(
                "合并结果包含 {} 个 error 诊断，已取消写入 {}",
                preview
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == ConfigDiagnosticSeverity::Error)
                    .count(),
                preview.summary.output_path.display()
            )));
        }

        let path = preview.summary.output_path.clone();
        atomic_write(&path, preview.yaml.as_bytes())
            .map_err(|error| ConfigError::InvalidDocument(format!("写入合并配置失败: {error}")))?;

        Ok(ConfigMergeWriteReport {
            path,
            bytes: preview.yaml.len(),
            summary: preview.summary,
            diagnostics: preview.diagnostics,
        })
    }
}

pub fn preview_config_merge(input: &ConfigMergeInput) -> Result<ConfigMergePreview, ConfigError> {
    ConfigMergePipeline::new().preview(input)
}

pub fn write_merged_config(
    input: &ConfigMergeInput,
) -> Result<ConfigMergeWriteReport, ConfigError> {
    ConfigMergePipeline::new().write_final_yaml(input)
}

struct MergeContext<'a> {
    document: MihomoConfigDocument,
    input: &'a ConfigMergeInput,
    diagnostics: Vec<ConfigDiagnostic>,
    summary: ConfigMergeSummary,
    policy_names: BTreeSet<String>,
    proxy_provider_names: BTreeSet<String>,
    rule_provider_names: BTreeSet<String>,
    sub_rule_names: BTreeSet<String>,
}

impl<'a> MergeContext<'a> {
    fn new(input: &'a ConfigMergeInput) -> Self {
        let document = input.profile.clone();
        let mut summary = ConfigMergeSummary {
            output_path: input.runtime.final_config_path(),
            profile_proxy_count: document.proxies.len(),
            ..ConfigMergeSummary::default()
        };
        summary.changes.push(ConfigMergeChange {
            stage: ConfigMergeStage::Profile,
            path: input
                .profile_id
                .as_deref()
                .map(|id| format!("profiles.{id}"))
                .unwrap_or_else(|| "profile".to_string()),
            kind: ConfigMergeChangeKind::Added,
            message: format!(
                "以当前 profile 为合并基线，包含 {} 个节点",
                document.proxies.len()
            ),
        });

        let policy_names = collect_policy_names(&document);
        let proxy_provider_names = document.proxy_providers.keys().cloned().collect();
        let rule_provider_names = document.rule_providers.keys().cloned().collect();
        let sub_rule_names = document.sub_rules.keys().cloned().collect();

        Self {
            document,
            input,
            diagnostics: Vec::new(),
            summary,
            policy_names,
            proxy_provider_names,
            rule_provider_names,
            sub_rule_names,
        }
    }

    /// 第二阶段：合并订阅缓存。
    ///
    /// 输入是每个订阅缓存里已解析的配置文档；输出是追加到主文档的节点、provider、
    /// 代理组和规则。订阅条目与当前文档重名时会被重命名，并同步改写同一订阅内的组和规则引用。
    fn merge_subscriptions(&mut self) {
        for source in &self.input.subscriptions {
            if !source.enabled {
                self.summary.changes.push(ConfigMergeChange {
                    stage: ConfigMergeStage::Subscription,
                    path: format!("subscriptions.{}", source.id),
                    kind: ConfigMergeChangeKind::Skipped,
                    message: "订阅已禁用，未参与运行配置合并".to_string(),
                });
                continue;
            }

            let mut incoming = source.document.clone();
            preserve_profile_runtime_globals(&self.input.profile, &mut self.document);
            let source_label = subscription_label(source);
            let proxy_renames = self.rename_incoming_proxies(source, &source_label, &mut incoming);
            let group_renames = self.rename_incoming_groups(source, &source_label, &mut incoming);
            let proxy_provider_renames =
                self.rename_proxy_providers(source, &source_label, &mut incoming);
            let rule_provider_renames =
                self.rename_rule_providers(source, &source_label, &mut incoming);
            let sub_rule_renames = self.rename_sub_rules(source, &source_label, &mut incoming);

            rewrite_references(
                &mut incoming,
                &proxy_renames,
                &group_renames,
                &proxy_provider_renames,
                &rule_provider_renames,
                &sub_rule_renames,
            );

            self.summary.subscription_proxy_count += incoming.proxies.len();
            self.document.proxies.extend(incoming.proxies);
            self.document.proxy_groups.extend(incoming.proxy_groups);
            self.document
                .proxy_providers
                .extend(incoming.proxy_providers);
            self.document.rule_providers.extend(incoming.rule_providers);
            self.document.rules.extend(incoming.rules);
            self.document.sub_rules.extend(incoming.sub_rules);
            preserve_profile_runtime_globals(&self.input.profile, &mut self.document);

            self.summary.changes.push(ConfigMergeChange {
                stage: ConfigMergeStage::Subscription,
                path: format!("subscriptions.{}", source.id),
                kind: ConfigMergeChangeKind::Added,
                message: format!("已合并订阅 `{}` 的缓存内容", source.display_name),
            });
        }
    }

    /// 第三阶段：应用 GUI 覆盖项。
    ///
    /// 输入是界面层收集的显式覆盖；输出是替换或追加后的主文档。覆盖项不做自动重命名，
    /// 因为用户显式命名应优先暴露冲突，最终由统一校验给出 error 诊断。
    fn apply_overrides(&mut self) {
        let overrides = &self.input.overrides;

        for proxy in &overrides.proxies {
            let replaced = replace_by_name(&mut self.document.proxies, proxy.clone(), |item| {
                item.name.as_str()
            });
            self.summary.changes.push(ConfigMergeChange {
                stage: ConfigMergeStage::Override,
                path: format!("overrides.proxies.{}", proxy.name),
                kind: if replaced {
                    ConfigMergeChangeKind::Replaced
                } else {
                    ConfigMergeChangeKind::Added
                },
                message: if replaced {
                    format!("GUI 覆盖替换了同名节点 `{}`", proxy.name)
                } else {
                    format!("GUI 覆盖新增节点 `{}`", proxy.name)
                },
            });
        }

        for group in &overrides.proxy_groups {
            let replaced =
                replace_by_name(&mut self.document.proxy_groups, group.clone(), |item| {
                    item.name.as_str()
                });
            self.summary.changes.push(ConfigMergeChange {
                stage: ConfigMergeStage::Override,
                path: format!("overrides.proxy-groups.{}", group.name),
                kind: if replaced {
                    ConfigMergeChangeKind::Replaced
                } else {
                    ConfigMergeChangeKind::Added
                },
                message: if replaced {
                    format!("GUI 覆盖替换了同名代理组 `{}`", group.name)
                } else {
                    format!("GUI 覆盖新增代理组 `{}`", group.name)
                },
            });
        }

        for (name, provider) in &overrides.proxy_providers {
            let replaced = self
                .document
                .proxy_providers
                .insert(name.clone(), provider.clone())
                .is_some();
            self.push_map_override_change("proxy-providers", name, replaced);
        }

        for (name, provider) in &overrides.rule_providers {
            let replaced = self
                .document
                .rule_providers
                .insert(name.clone(), provider.clone())
                .is_some();
            self.push_map_override_change("rule-providers", name, replaced);
        }

        match overrides.rule_mode {
            OverrideRuleMode::Append => self.document.rules.extend(overrides.rules.clone()),
            OverrideRuleMode::Replace => self.document.rules = overrides.rules.clone(),
        }
        if !overrides.rules.is_empty() {
            self.summary.changes.push(ConfigMergeChange {
                stage: ConfigMergeStage::Override,
                path: "overrides.rules".to_string(),
                kind: match overrides.rule_mode {
                    OverrideRuleMode::Append => ConfigMergeChangeKind::Added,
                    OverrideRuleMode::Replace => ConfigMergeChangeKind::Replaced,
                },
                message: format!(
                    "GUI 覆盖以 {:?} 模式应用了 {} 条主规则",
                    overrides.rule_mode,
                    overrides.rules.len()
                ),
            });
        }

        for (name, rules) in &overrides.sub_rules {
            let replaced = self
                .document
                .sub_rules
                .insert(name.clone(), rules.clone())
                .is_some();
            self.summary.changes.push(ConfigMergeChange {
                stage: ConfigMergeStage::Override,
                path: format!("overrides.sub-rules.{name}"),
                kind: if replaced {
                    ConfigMergeChangeKind::Replaced
                } else {
                    ConfigMergeChangeKind::Added
                },
                message: if replaced {
                    format!("GUI 覆盖替换了 sub-rules `{name}`")
                } else {
                    format!("GUI 覆盖新增 sub-rules `{name}`")
                },
            });
        }
    }

    /// 第四阶段：序列化预览并汇总合并结果。
    ///
    /// 输入是合并后的内存文档；输出是可展示 YAML、摘要计数和诊断列表。序列化在写盘前完成，
    /// 因此 YAML 生成失败也不会触碰 mihomo 当前工作配置。
    fn finish(mut self) -> Result<ConfigMergePreview, ConfigError> {
        self.summary.final_proxy_count = self.document.proxies.len();
        self.summary.final_group_count = self.document.proxy_groups.len();
        self.summary.final_rule_count = self.document.rules.len()
            + self
                .document
                .sub_rules
                .values()
                .map(Vec::len)
                .sum::<usize>();

        let yaml = serde_yaml::to_string(&self.document)
            .map_err(|error| ConfigError::InvalidDocument(error.to_string()))?;

        Ok(ConfigMergePreview {
            document: self.document,
            yaml,
            summary: self.summary,
            diagnostics: self.diagnostics,
        })
    }

    fn rename_incoming_proxies(
        &mut self,
        source: &SubscriptionMergeInput,
        source_label: &str,
        incoming: &mut MihomoConfigDocument,
    ) -> BTreeMap<String, String> {
        let mut renames = BTreeMap::new();
        for (index, proxy) in incoming.proxies.iter_mut().enumerate() {
            let path = format!("subscriptions.{}.proxies[{index}].name", source.id);
            let original = proxy.name.clone();
            if original.trim().is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "订阅节点名称不能为空",
                    Some("请修复订阅缓存或在 GUI 中为节点设置非空名称。".to_string()),
                ));
                continue;
            }

            let allocated = allocate_unique_name(
                &original,
                source_label,
                "节点",
                &mut self.policy_names,
                &path,
                &mut self.diagnostics,
                &mut self.summary,
            );
            if allocated != original {
                proxy.name = allocated.clone();
                renames.insert(original, allocated);
            }
        }
        renames
    }

    fn rename_incoming_groups(
        &mut self,
        source: &SubscriptionMergeInput,
        source_label: &str,
        incoming: &mut MihomoConfigDocument,
    ) -> BTreeMap<String, String> {
        let mut renames = BTreeMap::new();
        for (index, group) in incoming.proxy_groups.iter_mut().enumerate() {
            let path = format!("subscriptions.{}.proxy-groups[{index}].name", source.id);
            let original = group.name.clone();
            if original.trim().is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "订阅代理组名称不能为空",
                    Some("请修复订阅缓存或在 GUI 中为代理组设置非空名称。".to_string()),
                ));
                continue;
            }

            let allocated = allocate_unique_name(
                &original,
                source_label,
                "代理组",
                &mut self.policy_names,
                &path,
                &mut self.diagnostics,
                &mut self.summary,
            );
            if allocated != original {
                group.name = allocated.clone();
                renames.insert(original, allocated);
            }
        }
        renames
    }

    fn rename_proxy_providers(
        &mut self,
        source: &SubscriptionMergeInput,
        source_label: &str,
        incoming: &mut MihomoConfigDocument,
    ) -> BTreeMap<String, String> {
        rename_provider_map(
            &mut incoming.proxy_providers,
            &mut self.proxy_provider_names,
            source,
            source_label,
            "proxy provider",
            &mut self.diagnostics,
            &mut self.summary,
        )
    }

    fn rename_rule_providers(
        &mut self,
        source: &SubscriptionMergeInput,
        source_label: &str,
        incoming: &mut MihomoConfigDocument,
    ) -> BTreeMap<String, String> {
        rename_provider_map(
            &mut incoming.rule_providers,
            &mut self.rule_provider_names,
            source,
            source_label,
            "rule provider",
            &mut self.diagnostics,
            &mut self.summary,
        )
    }

    fn rename_sub_rules(
        &mut self,
        source: &SubscriptionMergeInput,
        source_label: &str,
        incoming: &mut MihomoConfigDocument,
    ) -> BTreeMap<String, String> {
        let mut renamed = BTreeMap::new();
        let mut next = BTreeMap::new();
        let old = std::mem::take(&mut incoming.sub_rules);
        for (name, rules) in old {
            let path = format!("subscriptions.{}.sub-rules.{name}", source.id);
            let allocated = allocate_unique_name(
                &name,
                source_label,
                "sub-rules",
                &mut self.sub_rule_names,
                &path,
                &mut self.diagnostics,
                &mut self.summary,
            );
            if allocated != name {
                renamed.insert(name, allocated.clone());
            }
            next.insert(allocated, rules);
        }
        incoming.sub_rules = next;
        renamed
    }

    fn push_map_override_change(&mut self, section: &str, name: &str, replaced: bool) {
        self.summary.changes.push(ConfigMergeChange {
            stage: ConfigMergeStage::Override,
            path: format!("overrides.{section}.{name}"),
            kind: if replaced {
                ConfigMergeChangeKind::Replaced
            } else {
                ConfigMergeChangeKind::Added
            },
            message: if replaced {
                format!("GUI 覆盖替换了 `{section}` 中的 `{name}`")
            } else {
                format!("GUI 覆盖新增了 `{section}` 中的 `{name}`")
            },
        });
    }
}

fn preserve_profile_runtime_globals(
    profile: &MihomoConfigDocument,
    document: &mut MihomoConfigDocument,
) {
    // 运行时控制面只能由当前 profile/启动装配决定。订阅缓存可能是完整 mihomo YAML，
    // 但其中的 controller 监听地址不能影响本机进程，否则 UI 客户端会连到错误端口。
    document.global.external_controller = profile.global.external_controller.clone();
    document.global.secret = profile.global.secret.clone();
}

fn collect_policy_names(document: &MihomoConfigDocument) -> BTreeSet<String> {
    let mut names = BUILTIN_POLICIES
        .iter()
        .map(|value| (*value).to_string())
        .collect::<BTreeSet<_>>();
    names.extend(document.proxies.iter().map(|proxy| proxy.name.clone()));
    names.extend(document.proxy_groups.iter().map(|group| group.name.clone()));
    names
}

fn subscription_label(source: &SubscriptionMergeInput) -> String {
    let display = source.display_name.trim();
    if display.is_empty() {
        source.id.clone()
    } else {
        display.to_string()
    }
}

fn allocate_unique_name(
    original: &str,
    source_label: &str,
    entity: &str,
    used: &mut BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    summary: &mut ConfigMergeSummary,
) -> String {
    if used.insert(original.to_string()) {
        return original.to_string();
    }

    let base = format!("{source_label} / {original}");
    let mut candidate = base.clone();
    let mut suffix = 2usize;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base} #{suffix}");
        suffix += 1;
    }

    diagnostics.push(ConfigDiagnostic::warning(
        path,
        format!("{entity} `{original}` 与现有策略名称冲突，已重命名为 `{candidate}`"),
        Some("如需固定名称，请在订阅或 GUI 覆盖中手动改名。".to_string()),
    ));
    summary.changes.push(ConfigMergeChange {
        stage: ConfigMergeStage::Subscription,
        path: path.to_string(),
        kind: ConfigMergeChangeKind::Renamed,
        message: format!("{entity} `{original}` 重命名为 `{candidate}`"),
    });

    candidate
}

fn rename_provider_map<T>(
    map: &mut BTreeMap<String, T>,
    used: &mut BTreeSet<String>,
    source: &SubscriptionMergeInput,
    source_label: &str,
    entity: &str,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    summary: &mut ConfigMergeSummary,
) -> BTreeMap<String, String> {
    let old = std::mem::take(map);
    let mut next = BTreeMap::new();
    let mut renamed = BTreeMap::new();
    for (name, provider) in old {
        let path = format!("subscriptions.{}.{entity}.{name}", source.id);
        let allocated = allocate_unique_name(
            &name,
            source_label,
            entity,
            used,
            &path,
            diagnostics,
            summary,
        );
        if allocated != name {
            renamed.insert(name, allocated.clone());
        }
        next.insert(allocated, provider);
    }
    *map = next;
    renamed
}

fn rewrite_references(
    document: &mut MihomoConfigDocument,
    proxy_renames: &BTreeMap<String, String>,
    group_renames: &BTreeMap<String, String>,
    proxy_provider_renames: &BTreeMap<String, String>,
    rule_provider_renames: &BTreeMap<String, String>,
    sub_rule_renames: &BTreeMap<String, String>,
) {
    let mut policy_renames = proxy_renames.clone();
    policy_renames.extend(
        group_renames
            .iter()
            .map(|(from, to)| (from.clone(), to.clone())),
    );

    for proxy in &mut document.proxies {
        rewrite_optional_policy(&mut proxy.dialer_proxy, &policy_renames);
    }

    for provider in document.proxy_providers.values_mut() {
        rewrite_optional_policy(&mut provider.proxy, &policy_renames);
        rewrite_optional_policy(&mut provider.dialer_proxy, &policy_renames);
    }

    for provider in document.rule_providers.values_mut() {
        rewrite_optional_policy(&mut provider.proxy, &policy_renames);
    }

    for group in &mut document.proxy_groups {
        for member in &mut group.proxies {
            rewrite_string(member, &policy_renames);
        }
        for provider in &mut group.use_providers {
            rewrite_string(provider, proxy_provider_renames);
        }
    }

    for rule in &mut document.rules {
        rewrite_rule_line(
            rule,
            &policy_renames,
            rule_provider_renames,
            sub_rule_renames,
        );
    }
    for rules in document.sub_rules.values_mut() {
        for rule in rules {
            rewrite_rule_line(
                rule,
                &policy_renames,
                rule_provider_renames,
                sub_rule_renames,
            );
        }
    }
}

fn rewrite_optional_policy(value: &mut Option<String>, renames: &BTreeMap<String, String>) {
    if let Some(name) = value {
        rewrite_string(name, renames);
    }
}

fn rewrite_string(value: &mut String, renames: &BTreeMap<String, String>) {
    if let Some(next) = renames.get(value.as_str()) {
        *value = next.clone();
    }
}

fn rewrite_rule_line(
    line: &mut RuleLine,
    policy_renames: &BTreeMap<String, String>,
    rule_provider_renames: &BTreeMap<String, String>,
    sub_rule_renames: &BTreeMap<String, String>,
) {
    let mut parts = split_rule_segments(&line.raw)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return;
    }

    let rule_type = parts[0].trim().replace('_', "-").to_ascii_uppercase();
    match rule_type.as_str() {
        "MATCH" => rewrite_part(&mut parts, 1, policy_renames),
        "RULE-SET" | "RULESET" => {
            rewrite_part(&mut parts, 1, rule_provider_renames);
            rewrite_part(&mut parts, 2, policy_renames);
        }
        "SUB-RULE" | "SUBRULE" => {
            let target_index = parts.len().saturating_sub(1);
            rewrite_part(&mut parts, target_index, sub_rule_renames);
        }
        _ => rewrite_part(&mut parts, 2, policy_renames),
    }

    line.raw = parts.join(",");
}

fn rewrite_part(parts: &mut [String], index: usize, renames: &BTreeMap<String, String>) {
    let Some(part) = parts.get_mut(index) else {
        return;
    };
    let trimmed = part.trim();
    if let Some(next) = renames.get(trimmed) {
        *part = next.clone();
    }
}

fn split_rule_segments(rule: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (index, ch) in rule.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&rule[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(&rule[start..]);
    parts
}

fn replace_by_name<T>(items: &mut Vec<T>, next: T, name: impl Fn(&T) -> &str) -> bool {
    let next_name = name(&next).to_string();
    if let Some(existing) = items.iter_mut().find(|item| name(item) == next_name) {
        *existing = next;
        true
    } else {
        items.push(next);
        false
    }
}

fn has_error_diagnostics(diagnostics: &[ConfigDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ConfigDiagnosticSeverity::Error)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "目标路径缺少父目录"))?;
    fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.flush()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_yaml::Value;

    use super::*;
    use air_config::ConfigDocument;
    use air_config::model::{ProviderKind, ProxyGroupKind, ProxyKind};

    fn runtime(temp: &tempfile::TempDir) -> ConfigMergeRuntimePaths {
        ConfigMergeRuntimePaths::new(temp.path().join("mihomo-work"))
    }

    fn proxy(name: &str) -> ProxyNode {
        ProxyNode {
            name: name.to_string(),
            kind: ProxyKind::Direct,
            ..ProxyNode::default()
        }
    }

    fn select_group(name: &str, proxies: &[&str]) -> ProxyGroup {
        ProxyGroup {
            name: name.to_string(),
            kind: ProxyGroupKind::Select,
            proxies: proxies.iter().map(|value| (*value).to_string()).collect(),
            ..ProxyGroup::default()
        }
    }

    #[test]
    fn renames_subscription_proxy_conflicts_and_rewrites_references() {
        let temp = tempfile::tempdir().unwrap();
        let mut profile = MihomoConfigDocument::default();
        profile.proxies.push(proxy("alpha"));
        profile
            .proxy_groups
            .push(select_group("Select", &["alpha"]));
        profile.rules.push(RuleLine {
            raw: "MATCH,Select".to_string(),
        });

        let mut subscription = MihomoConfigDocument::default();
        subscription.proxies.push(proxy("alpha"));
        subscription.proxies.push(proxy("beta"));
        subscription
            .proxy_groups
            .push(select_group("Auto", &["alpha", "beta", "DIRECT"]));
        subscription.rules.push(RuleLine {
            raw: "DOMAIN,example.com,alpha".to_string(),
        });
        subscription.rules.push(RuleLine {
            raw: "MATCH,Auto".to_string(),
        });

        let mut input = ConfigMergeInput::new(profile, runtime(&temp));
        input.subscriptions.push(SubscriptionMergeInput::enabled(
            "sub-a",
            "Sub A",
            subscription,
        ));

        let preview = preview_config_merge(&input).unwrap();

        assert!(
            preview
                .document
                .proxies
                .iter()
                .any(|item| item.name == "Sub A / alpha")
        );
        let auto = preview
            .document
            .proxy_groups
            .iter()
            .find(|group| group.name == "Auto")
            .unwrap();
        assert_eq!(auto.proxies[0], "Sub A / alpha");
        assert_eq!(
            preview.document.rules[1].raw,
            "DOMAIN,example.com,Sub A / alpha"
        );
        assert_eq!(preview.summary.subscription_proxy_count, 2);
        assert!(preview.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == ConfigDiagnosticSeverity::Warning
                && diagnostic.path == "subscriptions.sub-a.proxies[0].name"
        }));
        assert!(!preview.has_errors());
    }

    #[test]
    fn renames_provider_collisions_and_rewrites_group_and_rule_references() {
        let temp = tempfile::tempdir().unwrap();
        let mut profile = MihomoConfigDocument::default();
        profile.proxies.push(proxy("direct-a"));
        profile
            .proxy_providers
            .insert("remote".to_string(), ProxyProvider::default());
        profile
            .rule_providers
            .insert("rules".to_string(), RuleProvider::default());

        let mut subscription = MihomoConfigDocument::default();
        subscription.proxies.push(proxy("sub-node"));
        subscription.proxy_providers.insert(
            "remote".to_string(),
            ProxyProvider {
                kind: ProviderKind::Inline,
                payload: vec![proxy("from-provider")],
                ..ProxyProvider::default()
            },
        );
        subscription.rule_providers.insert(
            "rules".to_string(),
            RuleProvider {
                kind: ProviderKind::Inline,
                payload: vec!["DOMAIN,example.com,DIRECT".to_string()],
                ..RuleProvider::default()
            },
        );
        subscription
            .proxy_groups
            .push(select_group("UseProvider", &["sub-node"]));
        subscription.proxy_groups[0]
            .use_providers
            .push("remote".to_string());
        subscription.rules.push(RuleLine {
            raw: "RULE-SET,rules,UseProvider".to_string(),
        });

        let mut input = ConfigMergeInput::new(profile, runtime(&temp));
        input.subscriptions.push(SubscriptionMergeInput::enabled(
            "sub-a",
            "Sub A",
            subscription,
        ));

        let preview = preview_config_merge(&input).unwrap();

        let group = preview
            .document
            .proxy_groups
            .iter()
            .find(|group| group.name == "UseProvider")
            .unwrap();
        assert_eq!(group.use_providers, vec!["Sub A / remote".to_string()]);
        assert!(
            preview
                .document
                .proxy_providers
                .contains_key("Sub A / remote")
        );
        assert!(
            preview
                .document
                .rule_providers
                .contains_key("Sub A / rules")
        );
        assert_eq!(
            preview.document.rules[0].raw,
            "RULE-SET,Sub A / rules,UseProvider"
        );
        assert!(!preview.has_errors());
    }

    #[test]
    fn applies_gui_overrides_after_subscriptions() {
        let temp = tempfile::tempdir().unwrap();
        let mut profile = MihomoConfigDocument::default();
        profile.proxies.push(proxy("alpha"));

        let mut replacement = proxy("alpha");
        replacement.server = Some(Value::String("override.example".to_string()));

        let mut input = ConfigMergeInput::new(profile, runtime(&temp));
        input.overrides.proxies.push(replacement);
        input.overrides.rules.push(RuleLine {
            raw: "MATCH,DIRECT".to_string(),
        });

        let preview = preview_config_merge(&input).unwrap();

        assert_eq!(preview.document.proxies.len(), 1);
        assert_eq!(
            preview.document.proxies[0].server,
            Some(Value::String("override.example".to_string()))
        );
        assert_eq!(preview.document.rules[0].raw, "MATCH,DIRECT");
        assert!(preview.summary.changes.iter().any(|change| {
            change.stage == ConfigMergeStage::Override
                && change.kind == ConfigMergeChangeKind::Replaced
        }));
    }

    #[test]
    fn subscription_runtime_controller_never_overrides_profile_controller() {
        let temp = tempfile::tempdir().unwrap();
        let profile = ConfigDocument::parse(
            r#"
external-controller: 127.0.0.1:9090
secret: profile-secret
proxies:
  - name: profile-direct
    type: direct
"#,
        )
        .unwrap()
        .typed;
        let subscription = ConfigDocument::parse(
            r#"
external-controller: 127.0.0.1:19090
secret: subscription-secret
proxies:
  - name: sub-direct
    type: direct
proxy-groups:
  - name: SubSelect
    type: select
    proxies:
      - sub-direct
rules:
  - MATCH,SubSelect
"#,
        )
        .unwrap()
        .typed;

        let mut input = ConfigMergeInput::new(profile, runtime(&temp));
        input.subscriptions.push(SubscriptionMergeInput::enabled(
            "sub-a",
            "Sub A",
            subscription,
        ));

        let preview = preview_config_merge(&input).unwrap();

        assert_eq!(
            preview.document.global.external_controller.as_deref(),
            Some("127.0.0.1:9090")
        );
        assert_eq!(
            preview.document.global.secret.as_deref(),
            Some("profile-secret")
        );
        assert!(
            preview
                .document
                .proxies
                .iter()
                .any(|proxy| proxy.name == "sub-direct")
        );
        assert!(!preview.yaml.contains("127.0.0.1:19090"));
        assert!(!preview.yaml.contains("subscription-secret"));
    }

    #[test]
    fn merge_stage_error_does_not_overwrite_existing_runtime_config() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = runtime(&temp);
        let target = runtime.final_config_path();
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "mixed-port: 7890\n").unwrap();

        let profile = MihomoConfigDocument::default();
        let mut subscription = MihomoConfigDocument::default();
        subscription.proxies.push(ProxyNode::default());
        let input = ConfigMergeInput::new(profile, runtime);
        let mut input = input;
        input.subscriptions.push(SubscriptionMergeInput::enabled(
            "bad-sub",
            "Bad Sub",
            subscription,
        ));

        let error = write_merged_config(&input).unwrap_err();

        assert!(matches!(error, ConfigError::Validation(_)));
        assert_eq!(fs::read_to_string(&target).unwrap(), "mixed-port: 7890\n");
    }
}
