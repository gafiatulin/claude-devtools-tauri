// =============================================================================
// Message Tag Constants
// =============================================================================

pub const LOCAL_COMMAND_STDOUT_TAG: &str = "<local-command-stdout>";
pub const LOCAL_COMMAND_STDERR_TAG: &str = "<local-command-stderr>";
const LOCAL_COMMAND_CAVEAT_TAG: &str = "<local-command-caveat>";
const SYSTEM_REMINDER_TAG: &str = "<system-reminder>";
const TASK_NOTIFICATION_TAG: &str = "<task-notification>";

pub const EMPTY_STDOUT: &str = "<local-command-stdout></local-command-stdout>";
pub const EMPTY_STDERR: &str = "<local-command-stderr></local-command-stderr>";

pub const SYSTEM_OUTPUT_TAGS: &[&str] = &[
    LOCAL_COMMAND_STDERR_TAG,
    LOCAL_COMMAND_STDOUT_TAG,
    LOCAL_COMMAND_CAVEAT_TAG,
    SYSTEM_REMINDER_TAG,
    TASK_NOTIFICATION_TAG,
];

pub const HARD_NOISE_TAGS: &[&str] = &[
    LOCAL_COMMAND_CAVEAT_TAG,
    SYSTEM_REMINDER_TAG,
    TASK_NOTIFICATION_TAG,
];

// =============================================================================
// Trigger Color Definitions
// =============================================================================

pub struct TriggerColorDef {
    pub key: &'static str,
    pub label: &'static str,
    pub hex: &'static str,
}

pub const TRIGGER_COLORS: &[TriggerColorDef] = &[
    TriggerColorDef {
        key: "red",
        label: "Red",
        hex: "#ef4444",
    },
    TriggerColorDef {
        key: "orange",
        label: "Orange",
        hex: "#f97316",
    },
    TriggerColorDef {
        key: "yellow",
        label: "Yellow",
        hex: "#eab308",
    },
    TriggerColorDef {
        key: "green",
        label: "Green",
        hex: "#22c55e",
    },
    TriggerColorDef {
        key: "blue",
        label: "Blue",
        hex: "#3b82f6",
    },
    TriggerColorDef {
        key: "purple",
        label: "Purple",
        hex: "#a855f7",
    },
    TriggerColorDef {
        key: "pink",
        label: "Pink",
        hex: "#ec4899",
    },
    TriggerColorDef {
        key: "cyan",
        label: "Cyan",
        hex: "#06b6d4",
    },
];
