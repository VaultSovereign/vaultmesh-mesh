use std::collections::BTreeMap;
use std::env;
use std::process::Command;

#[derive(Default)]
pub struct EnvMetadata {
    pub ci: Option<String>,
    pub runner: Option<String>,
    pub entries: BTreeMap<String, String>,
}

// NOTE: This function aggregates many CI adapters and is expected to be long.
// We keep a single entrypoint for stable receipt shape. Refactors planned.
#[allow(clippy::too_many_lines)]
pub fn collect_env_metadata() -> EnvMetadata {
    let mut meta = EnvMetadata::default();

    if env::var("GITHUB_ACTIONS")
        .ok()
        .filter(|v| v == "true" || v == "1")
        .is_some()
    {
        meta.ci = Some("github_actions".into());
        insert(
            &mut meta.entries,
            "github_repository",
            env_var("GITHUB_REPOSITORY"),
        );
        insert(&mut meta.entries, "github_sha", env_var("GITHUB_SHA"));
        insert(&mut meta.entries, "github_ref", env_var("GITHUB_REF"));
        insert(&mut meta.entries, "github_run_id", env_var("GITHUB_RUN_ID"));
        insert(
            &mut meta.entries,
            "github_run_attempt",
            env_var("GITHUB_RUN_ATTEMPT"),
        );
        if let (Some(repo), Some(run_id)) = (
            meta.entries.get("github_repository"),
            meta.entries.get("github_run_id"),
        ) {
            let base = env_var("GITHUB_SERVER_URL").unwrap_or_else(|| "https://github.com".into());
            let base = base.trim_end_matches('/').to_string();
            let url = format!("{base}/{repo}/actions/runs/{run_id}");
            meta.entries.insert("workflow_url".into(), url.clone());
            meta.entries.insert("ci_url".into(), url);
        }
        if let Some(runner_name) = env_var("RUNNER_NAME") {
            meta.runner = Some(runner_name.clone());
            meta.entries
                .insert("github_runner_name".into(), runner_name);
        }
    } else if env::var("GITLAB_CI").is_ok() {
        meta.ci = Some("gitlab_ci".into());
        insert(
            &mut meta.entries,
            "gitlab_project",
            env_var("CI_PROJECT_PATH"),
        );
        insert(&mut meta.entries, "gitlab_sha", env_var("CI_COMMIT_SHA"));
        insert(
            &mut meta.entries,
            "gitlab_ref",
            env_var("CI_COMMIT_REF_NAME"),
        );
        insert(
            &mut meta.entries,
            "pipeline_url",
            env_var("CI_PIPELINE_URL"),
        );
        if let Some(u) = meta.entries.get("pipeline_url").cloned() {
            meta.entries.insert("ci_url".into(), u);
        }
    } else if env::var("CIRCLECI").is_ok() {
        meta.ci = Some("circleci".into());
        insert(
            &mut meta.entries,
            "circle_project",
            env_var("CIRCLE_PROJECT_REPONAME"),
        );
        insert(
            &mut meta.entries,
            "circle_username",
            env_var("CIRCLE_PROJECT_USERNAME"),
        );
        insert(&mut meta.entries, "circle_sha", env_var("CIRCLE_SHA1"));
        insert(&mut meta.entries, "circle_branch", env_var("CIRCLE_BRANCH"));
        insert(&mut meta.entries, "build_url", env_var("CIRCLE_BUILD_URL"));
        if let Some(u) = meta.entries.get("build_url").cloned() {
            meta.entries.insert("ci_url".into(), u);
        }
    } else if env::var("BUILDKITE").is_ok() {
        meta.ci = Some("buildkite".into());
        insert(
            &mut meta.entries,
            "buildkite_org",
            env_var("BUILDKITE_ORGANIZATION_SLUG"),
        );
        insert(
            &mut meta.entries,
            "buildkite_pipeline",
            env_var("BUILDKITE_PIPELINE_SLUG"),
        );
        insert(
            &mut meta.entries,
            "buildkite_build_number",
            env_var("BUILDKITE_BUILD_NUMBER"),
        );
        insert(
            &mut meta.entries,
            "buildkite_commit",
            env_var("BUILDKITE_COMMIT"),
        );
        insert(
            &mut meta.entries,
            "build_url",
            env_var("BUILDKITE_BUILD_URL"),
        );
        if let Some(u) = meta.entries.get("build_url").cloned() {
            meta.entries.insert("ci_url".into(), u);
        }
    } else if env::var("JENKINS_URL").is_ok() {
        meta.ci = Some("jenkins".into());
        insert(&mut meta.entries, "jenkins_url", env_var("JENKINS_URL"));
        insert(&mut meta.entries, "jenkins_job", env_var("JOB_NAME"));
        insert(
            &mut meta.entries,
            "jenkins_build_number",
            env_var("BUILD_NUMBER"),
        );
        insert(&mut meta.entries, "jenkins_build_tag", env_var("BUILD_TAG"));
        if let Some(u) = env_var("BUILD_URL") {
            meta.entries.insert("ci_url".into(), u.clone());
            insert(&mut meta.entries, "build_url", Some(u));
        }
    } else if env::var("AZURE_HTTP_USER_AGENT").is_ok() {
        meta.ci = Some("azure_pipelines".into());
        insert(
            &mut meta.entries,
            "azure_definition",
            env_var("BUILD_DEFINITIONNAME"),
        );
        insert(
            &mut meta.entries,
            "azure_build_id",
            env_var("BUILD_BUILDID"),
        );
        insert(
            &mut meta.entries,
            "azure_repo",
            env_var("BUILD_REPOSITORY_NAME"),
        );
        insert(
            &mut meta.entries,
            "azure_source_branch",
            env_var("BUILD_SOURCEBRANCH"),
        );
        insert(&mut meta.entries, "build_url", env_var("BUILD_BUILDURI"));
        if let Some(u) = meta.entries.get("build_url").cloned() {
            meta.entries.insert("ci_url".into(), u);
        }
    }

    let runner_hint = env_var("HOSTNAME").or_else(|| env_var("COMPUTERNAME"));
    if meta.runner.is_none() {
        meta.runner.clone_from(&runner_hint);
    }
    if let Some(runner) = runner_hint {
        insert(&mut meta.entries, "runner", Some(runner));
    }

    if let Some(commit) = env_var("GIT_COMMIT")
        .or_else(|| meta.entries.get("github_sha").cloned())
        .or_else(|| meta.entries.get("gitlab_sha").cloned())
        .or_else(|| meta.entries.get("circle_sha").cloned())
        .or_else(|| meta.entries.get("buildkite_commit").cloned())
    {
        meta.entries.insert("git_commit".into(), commit);
    }

    if let Some(reference) = env_var("GIT_REF")
        .or_else(|| meta.entries.get("github_ref").cloned())
        .or_else(|| meta.entries.get("gitlab_ref").cloned())
        .or_else(|| meta.entries.get("circle_branch").cloned())
        .or_else(|| meta.entries.get("azure_source_branch").cloned())
    {
        meta.entries.insert("git_ref".into(), reference);
    }

    if let Some(tfv) = detect_terraform_version() {
        meta.entries.insert("terraform_version".into(), tfv);
    }

    meta
}

pub fn detect_terraform_version() -> Option<String> {
    if let Some(v) = env_var("VM_TF_VERSION") {
        return Some(v);
    }
    let output = Command::new("terraform").arg("version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_terraform_version_from_stdout(&stdout)
}

fn env_var(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn insert(map: &mut BTreeMap<String, String>, key: &str, value: Option<String>) {
    if let Some(v) = value {
        map.insert(key.to_string(), v);
    }
}

fn parse_terraform_version_from_stdout(s: &str) -> Option<String> {
    let line = s
        .lines()
        .find(|l| l.to_ascii_lowercase().contains("terraform v"))
        .unwrap_or_else(|| s.trim());
    let bytes = line.as_bytes();
    let mut start = None;
    for (i, ch) in bytes.iter().enumerate() {
        let c = *ch as char;
        if c == 'v' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit() {
            start = Some(i + 1);
            break;
        }
        if c.is_ascii_digit() {
            start = Some(i);
            break;
        }
    }
    let idx = start?;
    let mut out = String::new();
    for ch in line[idx..].chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch.is_ascii_alphanumeric()
        {
            out.push(ch);
        } else {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tf_parse_simple() {
        assert_eq!(
            parse_terraform_version_from_stdout("Terraform v1.6.6"),
            Some("1.6.6".into())
        );
    }

    #[test]
    fn tf_parse_prerelease() {
        assert_eq!(
            parse_terraform_version_from_stdout("Terraform v1.9.2-beta2 on darwin_amd64"),
            Some("1.9.2-beta2".into())
        );
    }

    #[test]
    fn tf_parse_noisy() {
        let s = "Terraform v1.5.7\non linux_amd64\nother tools...";
        assert_eq!(parse_terraform_version_from_stdout(s), Some("1.5.7".into()));
    }

    #[test]
    fn tf_parse_with_plus_build() {
        let s = "Hashi Terraform v1.7.0+ent on linux_x86_64";
        assert_eq!(
            parse_terraform_version_from_stdout(s),
            Some("1.7.0+ent".into())
        );
    }
}
