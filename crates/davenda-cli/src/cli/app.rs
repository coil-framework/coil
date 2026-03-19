use crate::CliModelError;
use crate::cli::args::{CliInput, parse};
use crate::cli::auth::execute_auth_explain;
use crate::cli::error::CliRunError;
use crate::cli::render::render_auth_explain;
use crate::command::CommandInvocation;
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
            customer_app,
            output_mode,
            invocation,
        } => {
            let app = CliApplication::new(customer_app)?;
            let _plan = app
                .runtime
                .plan(CommandInvocation::new(["auth", "explain"])?.with_output_mode(output_mode))?;

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
        "  platform auth explain --subject <subject> --capability <capability> --resource <namespace:id> [--tuple <object#relation=subject>]... [--json]",
        "",
        "Examples:",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage --tuple page:homepage#site=site:main --tuple site:main#viewer=user:alice",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_from_args_dispatches_auth_explain_end_to_end() {
        let rendered = run_from_args([
            "auth".to_string(),
            "explain".to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
            "--tuple".to_string(),
            "page:homepage#site=site:main".to_string(),
            "--tuple".to_string(),
            "site:main#viewer=user:alice".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("decision: allow"));
        assert!(rendered.contains("page:homepage"));
    }
}
