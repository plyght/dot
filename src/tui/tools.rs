#[derive(Debug, Clone, PartialEq)]
pub enum ToolCategory {
    FileRead,
    FileWrite,
    MultiEdit,
    Directory,
    Search,
    Command,
    Glob,
    Grep,
    WebFetch,
    Patch,
    Batch,
    Snapshot,
    Question,
    Mcp { server: String },
    Skill,
    Unknown,
}

impl ToolCategory {
    pub fn from_name(name: &str) -> Self {
        match name {
            "read_file" => Self::FileRead,
            "write_file" => Self::FileWrite,
            "multiedit" => Self::MultiEdit,
            "list_directory" => Self::Directory,
            "search_files" => Self::Search,
            "run_command" => Self::Command,
            "glob" => Self::Glob,
            "grep" => Self::Grep,
            "webfetch" => Self::WebFetch,
            "apply_patch" => Self::Patch,
            "batch" => Self::Batch,
            "snapshot_list" | "snapshot_restore" => Self::Snapshot,
            "question" => Self::Question,
            "skill" => Self::Skill,
            other => {
                if let Some(idx) = other.find('_') {
                    let prefix = &other[..idx];
                    if ![
                        "read", "write", "list", "search", "run", "snapshot", "apply",
                    ]
                    .contains(&prefix)
                    {
                        return Self::Mcp {
                            server: prefix.to_string(),
                        };
                    }
                }
                Self::Unknown
            }
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::FileRead => "\u{f15c} ",
            Self::FileWrite => "\u{270e} ",
            Self::MultiEdit => "\u{270e} ",
            Self::Directory => "\u{f07b} ",
            Self::Search => "\u{f002} ",
            Self::Command => "\u{f120} ",
            Self::Glob => "\u{f002} ",
            Self::Grep => "\u{f002} ",
            Self::WebFetch => "\u{f0ac} ",
            Self::Patch => "\u{270e} ",
            Self::Batch => "\u{f0c2} ",
            Self::Snapshot => "\u{f0c2} ",
            Self::Question => "\u{f128} ",
            Self::Mcp { .. } => "\u{f1e6} ",
            Self::Skill => "\u{f0eb} ",
            Self::Unknown => "\u{f013} ",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::FileRead => "read".to_string(),
            Self::FileWrite => "write".to_string(),
            Self::MultiEdit => "edit".to_string(),
            Self::Directory => "list".to_string(),
            Self::Search => "search".to_string(),
            Self::Command => "run".to_string(),
            Self::Glob => "glob".to_string(),
            Self::Grep => "grep".to_string(),
            Self::WebFetch => "fetch".to_string(),
            Self::Patch => "patch".to_string(),
            Self::Batch => "batch".to_string(),
            Self::Snapshot => "snapshot".to_string(),
            Self::Question => "question".to_string(),
            Self::Mcp { server } => format!("mcp:{}", server),
            Self::Skill => "skill".to_string(),
            Self::Unknown => "tool".to_string(),
        }
    }

    pub fn intent(&self) -> &'static str {
        match self {
            Self::FileRead => "reading",
            Self::FileWrite => "writing",
            Self::MultiEdit => "editing",
            Self::Directory => "listing",
            Self::Search => "searching",
            Self::Command => "running",
            Self::Glob => "finding",
            Self::Grep => "searching",
            Self::WebFetch => "fetching",
            Self::Patch => "patching",
            Self::Batch => "running",
            Self::Snapshot => "checking",
            Self::Question => "asking",
            Self::Mcp { .. } => "calling",
            Self::Skill => "loading",
            Self::Unknown => "running",
        }
    }
}

pub struct ToolCallDisplay {
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub is_error: bool,
    pub category: ToolCategory,
    pub detail: String,
}

pub fn extract_tool_detail(name: &str, input: &str) -> String {
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(input);
    let val = match parsed {
        Ok(v) => v,
        Err(_) => return String::new(),
    };

    match name {
        "read_file" => val
            .get("path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_default(),
        "write_file" => val
            .get("path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_default(),
        "list_directory" => val
            .get("path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_default(),
        "search_files" => {
            let pattern = val.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = val.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                format!("\"{}\"", pattern)
            } else {
                format!("\"{}\" in {}", pattern, shorten_path(path))
            }
        }
        "run_command" => val
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| {
                if c.len() > 60 {
                    format!("{}...", &c[..57])
                } else {
                    c.to_string()
                }
            })
            .unwrap_or_default(),
        "glob" => val
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "grep" => {
            let pattern = val.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = val.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                format!("\"{}\"", pattern)
            } else {
                format!("\"{}\"; in {}", pattern, shorten_path(path))
            }
        }
        "webfetch" => val
            .get("url")
            .and_then(|v| v.as_str())
            .map(|u| {
                if u.len() > 60 {
                    format!("{}...", &u[..57])
                } else {
                    u.to_string()
                }
            })
            .unwrap_or_default(),
        "apply_patch" => {
            let count = val
                .get("patches")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("{} patches", count)
        }
        "multiedit" => {
            let path = val
                .get("path")
                .and_then(|v| v.as_str())
                .map(shorten_path)
                .unwrap_or_default();
            let count = val
                .get("edits")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("{} ({} edits)", path, count)
        }
        "batch" => {
            let count = val
                .get("invocations")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("{} tools", count)
        }
        "snapshot_list" => "listing changes".to_string(),
        "snapshot_restore" => val
            .get("path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_else(|| "all files".to_string()),
        "question" => val
            .get("question")
            .and_then(|v| v.as_str())
            .map(|q| {
                if q.len() > 50 {
                    format!("{}...", &q[..47])
                } else {
                    q.to_string()
                }
            })
            .unwrap_or_default(),
        "skill" => val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => {
            if let Some(first_str) = val
                .as_object()
                .and_then(|o| o.values().find_map(|v| v.as_str().map(|s| s.to_string())))
            {
                if first_str.len() > 50 {
                    format!("{}...", &first_str[..47])
                } else {
                    first_str
                }
            } else {
                String::new()
            }
        }
    }
}

fn shorten_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME")
        && let Some(rest) = path.strip_prefix(&home)
    {
        return format!("~{}", rest);
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_str = cwd.to_string_lossy();
        if let Some(rest) = path.strip_prefix(cwd_str.as_ref()) {
            let rest = rest.strip_prefix('/').unwrap_or(rest);
            return if rest.is_empty() {
                ".".to_string()
            } else {
                format!("./{}", rest)
            };
        }
    }
    path.to_string()
}
