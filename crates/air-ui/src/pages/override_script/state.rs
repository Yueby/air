use air_app::AppCommand;
use air_config::DEFAULT_OVERRIDE_SCRIPT;
#[derive(Clone, Debug)]
pub(crate) struct OverrideScriptPageState {
    enabled: bool,
    script: String,
    saved_script: String,
    preview_modal: OverridePreviewModalState,
}

impl OverrideScriptPageState {
    pub(crate) fn new(enabled: bool, script: String) -> Self {
        Self {
            enabled,
            saved_script: script.clone(),
            script,
            preview_modal: OverridePreviewModalState::Closed,
        }
    }

    pub(crate) fn set_script(&mut self, script: impl Into<String>) {
        self.script = script.into();
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) -> AppCommand {
        self.enabled = enabled;
        AppCommand::SetOverrideScriptEnabled { enabled }
    }

    pub(crate) fn save(&mut self) -> AppCommand {
        self.saved_script = self.script.clone();
        AppCommand::SaveOverrideScript {
            script: self.script.clone(),
            enabled: self.enabled,
        }
    }

    pub(crate) fn debug(&mut self) -> AppCommand {
        self.preview_modal = OverridePreviewModalState::Loading;
        AppCommand::DebugOverrideScript {
            script: self.script.clone(),
        }
    }

    pub(crate) fn set_preview(&mut self, contents: String) {
        self.preview_modal = OverridePreviewModalState::Ready { contents };
    }

    pub(crate) fn set_preview_error(&mut self, message: String) {
        self.preview_modal = OverridePreviewModalState::Error { message };
    }

    pub(crate) fn close_preview(&mut self) {
        self.preview_modal = OverridePreviewModalState::Closed;
    }

    pub(crate) fn preview_contents(&self) -> &str {
        self.preview_modal.contents()
    }

    pub(crate) fn preview_is_open(&self) -> bool {
        self.preview_modal.is_open()
    }

    pub(crate) fn view_model(&self) -> OverrideScriptPageViewModel {
        OverrideScriptPageViewModel {
            enabled: self.enabled,
            dirty: self.script != self.saved_script,
            preview_modal: self.preview_modal.clone(),
        }
    }
}

impl Default for OverrideScriptPageState {
    fn default() -> Self {
        Self::new(false, DEFAULT_OVERRIDE_SCRIPT.to_string())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OverrideScriptPageViewModel {
    pub(crate) enabled: bool,
    pub(crate) dirty: bool,
    pub(crate) preview_modal: OverridePreviewModalState,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum OverridePreviewModalState {
    #[default]
    Closed,
    Loading,
    Ready {
        contents: String,
    },
    Error {
        message: String,
    },
}

impl OverridePreviewModalState {
    pub(crate) fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }

    pub(crate) fn contents(&self) -> &str {
        match self {
            Self::Ready { contents } => contents,
            Self::Error { message } => message,
            Self::Loading => "# 正在生成运行配置预览\n",
            Self::Closed => "",
        }
    }
}
