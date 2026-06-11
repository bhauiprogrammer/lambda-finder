use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const REGION: &str = "ap-south-1";

fn lambda_base() -> String {
    format!(
        "https://{region}.console.aws.amazon.com/lambda/home?region={region}#/functions/",
        region = REGION
    )
}

fn logs_base() -> String {
    format!(
        "https://{region}.console.aws.amazon.com/cloudwatch/home?region={region}#logsV2:log-groups/log-group/",
        region = REGION
    )
}

#[derive(Debug, Clone)]
pub struct Repo {
    pub key: &'static str,
    pub folder: &'static str,
    pub dev: &'static str,
    pub stage: &'static str,
    pub preprod: &'static str,
}

impl Repo {
    fn prefix(&self, env: &str) -> Option<&'static str> {
        match env {
            "dev" => Some(self.dev),
            "stage" => Some(self.stage),
            "preprod" => Some(self.preprod),
            _ => None,
        }
    }
}

pub static REPOS: Lazy<Vec<Repo>> = Lazy::new(|| {
    vec![
        Repo {
            key: "svc",
            folder: "service-requests-backend",
            dev: "service-requests-dev-",
            stage: "service-requests-stage-",
            preprod: "service-requests-preprod-",
        },
        Repo {
            key: "ms",
            folder: "utec-microservices",
            dev: "utec-microservices-test-",
            stage: "utec-microservices-stage-",
            preprod: "utec-micro-preprod-",
        },
        Repo {
            key: "onetechnical",
            folder: "polaris-backend",
            dev: "polaris-tasc-panel-dev-",
            stage: "polaris-tasc-panel-stage-",
            preprod: "polaris-tasc-panel-preprod-",
        },
        Repo {
            key: "bpd",
            folder: "bpd-qrc-backend",
            dev: "bpd-qrc-dev-env-",
            stage: "bpd-qrc-stage-env-",
            preprod: "bpd-qrc-pre-prod-",
        },
        Repo {
            key: "srt",
            folder: "service-sr-tracker",
            dev: "sr-tracker-test-",
            stage: "sr-tracker-staging-",
            preprod: "sr-tracker-preprod-",
        },
        Repo {
            key: "user",
            folder: "service-user-onboarding",
            dev: "utec-user-onboarding-test-",
            stage: "utec-user-onboarding-stage-",
            preprod: "utec-user-onboarding-preprod-",
        },
    ]
});

fn folder_to_repo() -> HashMap<&'static str, &'static Repo> {
    REPOS.iter().map(|r| (r.folder, r)).collect()
}

#[derive(Debug, Serialize, Clone)]
pub struct Match {
    pub repo: String,
    pub folder: String,
    pub file: String,
    #[serde(rename = "functionName")]
    pub function_name: String,
    #[serde(rename = "explicitFunctionName")]
    pub explicit_function_name: Option<String>,
    #[serde(rename = "lambdaName")]
    pub lambda_name: String,
    #[serde(rename = "logGroup")]
    pub log_group: String,
    #[serde(rename = "lambdaUrl")]
    pub lambda_url: String,
    #[serde(rename = "logsUrl")]
    pub logs_url: String,
}

#[derive(Debug, Serialize)]
pub struct FindResult {
    pub matches: Vec<Match>,
    pub warnings: Vec<String>,
    #[serde(rename = "searchedFolders")]
    pub searched_folders: Vec<String>,
}

static RE_PATH_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*Path:\s*(\S+)").unwrap());
static RE_TWO_SPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^ {2}\S").unwrap());
static RE_LOGS_GROUP_SUFFIX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)Logs?Group$").unwrap());
static RE_FUNCTION_NAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*FunctionName:\s*(.+?)\s*$").unwrap());
static RE_LOG_GROUP_NAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"LogGroupName:\s*(.+?)\s*$").unwrap());
static RE_ENV_STACKNAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"envStackname-?").unwrap());
static RE_SUB_PREFIX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^!Sub\s+").unwrap());
static RE_OUTER_QUOTES: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^["']|["']$"#).unwrap());
static RE_SUB_PLACEHOLDER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^.*\$\{[^}]+\}(.*)$").unwrap());
static RE_LEADING_DASHES: Lazy<Regex> = Lazy::new(|| Regex::new(r"^-+").unwrap());

struct YamlMatch {
    lambda_function_name: String,
    log_group_name: Option<String>,
    explicit_function_name: Option<String>,
}

// Look for the function whose Events.Api Path contains <needle>.
// Walking is bottom-up but ONLY anchored on `Path:` lines, so we can't
// accidentally land inside a sibling LogGroup resource that mentions the name.
fn search_yaml(file_path: &Path, needle: &str) -> Option<YamlMatch> {
    let content = fs::read_to_string(file_path).ok()?;
    let lines: Vec<&str> = content.split('\n').collect();
    let needle_lower = needle.to_lowercase();

    let mut lambda_function_name: Option<String> = None;
    let mut lambda_function_line_idx: Option<usize> = None;

    'outer: for i in (0..lines.len()).rev() {
        let line = lines[i];
        let caps = match RE_PATH_LINE.captures(line) {
            Some(c) => c,
            None => continue,
        };
        let path_value = caps.get(1).unwrap().as_str();
        if !path_value.to_lowercase().contains(&needle_lower) {
            continue;
        }

        for j in (0..i).rev() {
            if RE_TWO_SPACE.is_match(lines[j]) {
                let name = lines[j].trim().trim_end_matches(':').trim().to_string();
                if RE_LOGS_GROUP_SUFFIX.is_match(&name) {
                    continue;
                }
                lambda_function_name = Some(name);
                lambda_function_line_idx = Some(j);
                break 'outer;
            }
        }
    }

    let lambda_function_name = lambda_function_name?;

    // Walk forward from the resource's logical name line to capture an explicit
    // `FunctionName:` directive within the same resource block.
    let mut explicit_function_name: Option<String> = None;
    if let Some(start) = lambda_function_line_idx {
        for k in (start + 1)..lines.len() {
            if RE_TWO_SPACE.is_match(lines[k]) {
                break;
            }
            if let Some(caps) = RE_FUNCTION_NAME.captures(lines[k]) {
                explicit_function_name = Some(caps.get(1).unwrap().as_str().trim().to_string());
                break;
            }
        }
    }

    // Locate the LogGroup resource (uses two header variants).
    let headers = [
        format!("  {}LogsGroup:", lambda_function_name),
        format!("  {}LogGroup:", lambda_function_name),
    ];
    let mut log_group_name: Option<String> = None;
    'lg: for i in 0..lines.len() {
        for header in &headers {
            if lines[i].starts_with(header.as_str()) {
                let end = (i + 15).min(lines.len());
                for k in i..end {
                    if let Some(caps) = RE_LOG_GROUP_NAME.captures(lines[k]) {
                        log_group_name = Some(caps.get(1).unwrap().as_str().trim().to_string());
                        break;
                    }
                }
                break 'lg;
            }
        }
    }

    Some(YamlMatch {
        lambda_function_name,
        log_group_name,
        explicit_function_name,
    })
}

// Resolve a literal Lambda name from a YAML `FunctionName:` directive.
// Handles two conventions used in these templates:
//   1. SAM `!Sub` with `${...}` placeholders (env-aware: stack_prefix + sub_stack)
//   2. Literal `envStackname-` placeholder (utec-microservices convention) —
//      maps to just `stack_prefix` since it is the deployed stack name only.
fn resolve_explicit_function_name(
    template: &str,
    stack_prefix: &str,
    sub_stack: &str,
) -> String {
    let trimmed = template.trim();
    let no_sub = RE_SUB_PREFIX.replace(trimmed, "").to_string();
    let unquoted = RE_OUTER_QUOTES.replace_all(&no_sub, "").to_string();

    if RE_ENV_STACKNAME.is_match(&unquoted) {
        return RE_ENV_STACKNAME
            .replace(&unquoted, stack_prefix)
            .to_string();
    }

    if let Some(caps) = RE_SUB_PLACEHOLDER.captures(&unquoted) {
        let suffix = caps.get(1).unwrap().as_str();
        let stripped = RE_LEADING_DASHES.replace(suffix, "").to_string();
        return format!("{}{}{}", stack_prefix, sub_stack, stripped);
    }

    unquoted
}

fn encode_log_group(name: &str) -> String {
    name.replace('/', "$252F")
}

fn top_level_folder(rel: &str) -> &str {
    let cleaned = rel.strip_prefix("./").unwrap_or(rel);
    cleaned.split('/').next().unwrap_or(cleaned)
}

pub fn find_matches(repo_root: &str, endpoint: &str, env: &str) -> Result<FindResult, String> {
    if repo_root.is_empty() {
        return Err("repoRoot is required".into());
    }
    let needle = endpoint.trim();
    if needle.is_empty() {
        return Err("Endpoint is required".into());
    }
    if !matches!(env, "dev" | "stage" | "preprod") {
        return Err("Env must be one of: dev, stage, preprod".into());
    }

    let root_path = PathBuf::from(repo_root);
    if !root_path.exists() {
        return Err(format!("Repo root does not exist: {}", repo_root));
    }

    let mut folders: Vec<&str> = Vec::new();
    for r in REPOS.iter() {
        if root_path.join(r.folder).exists() {
            folders.push(r.folder);
        }
    }

    if folders.is_empty() {
        let known: Vec<&str> = REPOS.iter().map(|r| r.folder).collect();
        return Err(format!(
            "None of the known repo folders exist in {}.\nExpected one or more of: {}",
            repo_root,
            known.join(", ")
        ));
    }

    let mut cmd = Command::new("grep");
    cmd.current_dir(&root_path)
        .arg("-i")
        .arg("-r")
        .arg("-l")
        .arg("--include=*.yml")
        .arg("--include=*.yaml")
        .arg(needle);
    for folder in &folders {
        cmd.arg(format!("{}/", folder));
    }

    let output = cmd.output().map_err(|e| format!("failed to run grep: {}", e))?;

    // grep exit code 1 = no matches; both 0 and 1 are OK. >1 is a real error.
    if let Some(code) = output.status.code() {
        if code > 1 {
            return Err(format!(
                "grep failed (exit {}): {}",
                code,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<&str> = stdout
        .split('\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut matches: Vec<Match> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if files.is_empty() {
        warnings.push(format!(
            r#"No yml files contain "{}" in any known repo."#,
            needle
        ));
        return Ok(FindResult {
            matches,
            warnings,
            searched_folders: folders.iter().map(|s| s.to_string()).collect(),
        });
    }

    let map = folder_to_repo();
    for rel_file in &files {
        let folder = top_level_folder(rel_file);
        let repo = match map.get(folder) {
            Some(r) => *r,
            None => {
                warnings.push(format!(
                    r#"Skipping {}: folder "{}" not in repo map"#,
                    rel_file, folder
                ));
                continue;
            }
        };

        let stack_prefix = match repo.prefix(env) {
            Some(p) => p,
            None => {
                warnings.push(format!(
                    r#"Env "{}" not configured for repo "{}". Skipping {}."#,
                    env, repo.key, rel_file
                ));
                continue;
            }
        };

        let abs_file = root_path.join(rel_file);
        let yml = abs_file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let sub_stack = if yml == "template.yml" || yml == "template.yaml" {
            String::new()
        } else {
            format!("{}-", yml.split('-').next().unwrap_or(""))
        };

        let yaml_match = match search_yaml(&abs_file, needle) {
            Some(m) => m,
            None => {
                warnings.push(format!(
                    r#"No Path containing "{}" found in {}; skipping."#,
                    needle, rel_file
                ));
                continue;
            }
        };

        let real_name = if let Some(template) = yaml_match.explicit_function_name.as_deref() {
            resolve_explicit_function_name(template, stack_prefix, &sub_stack)
        } else {
            format!(
                "{}{}{}",
                stack_prefix, sub_stack, yaml_match.lambda_function_name
            )
        };

        let real_log_group = match yaml_match.log_group_name.as_deref() {
            Some(lg) => RE_ENV_STACKNAME.replace(lg, stack_prefix).to_string(),
            None => format!("/aws/lambda/{}", real_name),
        };

        let lambda_url = format!("{}{}?tab=code", lambda_base(), real_name);
        let logs_url = format!("{}{}", logs_base(), encode_log_group(&real_log_group));

        matches.push(Match {
            repo: repo.key.to_string(),
            folder: folder.to_string(),
            file: rel_file.to_string(),
            function_name: yaml_match.lambda_function_name.clone(),
            explicit_function_name: yaml_match.explicit_function_name.clone(),
            lambda_name: real_name,
            log_group: real_log_group,
            lambda_url,
            logs_url,
        });
    }

    Ok(FindResult {
        matches,
        warnings,
        searched_folders: folders.iter().map(|s| s.to_string()).collect(),
    })
}
