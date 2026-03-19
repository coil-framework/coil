use crate::CliModelError;
use crate::cli::args::{CliInput, parse};
use crate::cli::auth::execute_auth_explain;
use crate::cli::error::CliRunError;
use crate::cli::render::render_auth_explain;
use crate::registry::CliRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliApplication {
    runtime: CliRuntime,
}

impl CliApplication {
    pub fn new(customer_app: impl Into<String>) -> Result<Self, CliModelError> {
        Ok(Self {
            runtime: CliRuntime::baseline(customer_app)?,
        })
    }

    pub fn runtime(&self) -> &CliRuntime {
        &self.runtime
    }
}

pub fn run_from_args(args: impl IntoIterator<Item = String>) -> Result<String, CliRunError> {
    let input = parse(args)?;
    match input {
        CliInput::Help => Ok(usage()),
        CliInput::AuthExplain {
            output_mode,
            invocation,
        } => {
            let result = execute_auth_explain(invocation)?;
            render_auth_explain(&result, output_mode)
        }
    }
}

pub fn run_from_env() -> i32 {
    match run_from_args(std::env::args().skip(1)) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn usage() -> String {
    [
        "Usage:",
        "  platform auth explain --config <path> --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "",
        "Examples:",
        "  platform auth explain --config ./platform.toml --subject user:alice --capability cms.page.publish --resource page:homepage",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_from_args_returns_usage_for_help() {
        let rendered = run_from_args(["--help".to_string()]).unwrap();
        assert!(rendered.contains("platform auth explain --config <path>"));
    }
}
