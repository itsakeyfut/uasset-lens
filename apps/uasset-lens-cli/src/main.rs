use std::io::Write;

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};

fn main() {
    let cli = cli::Cli::parse();
    if let cli::Commands::Completions { shell } = &cli.command {
        if let Err(msg) = generate_completions(shell, &mut std::io::stdout()) {
            eprintln!("{msg}");
            std::process::exit(2);
        }
        return;
    }
    std::process::exit(cli::run_with(cli));
}

fn generate_completions(shell_str: &str, out: &mut impl Write) -> Result<(), String> {
    let shell = shell_str.parse::<Shell>().map_err(|_| {
        format!(
            "Error: unknown shell '{}'. Supported: bash, zsh, fish, powershell, elvish",
            shell_str
        )
    })?;
    let mut cmd = cli::Cli::command();
    generate(shell, &mut cmd, "uasset-lens", out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_completions_should_produce_output_for_bash() {
        let mut buf = Vec::new();
        assert!(generate_completions("bash", &mut buf).is_ok());
        assert!(!buf.is_empty());
    }

    #[test]
    fn generate_completions_should_produce_output_for_zsh() {
        let mut buf = Vec::new();
        assert!(generate_completions("zsh", &mut buf).is_ok());
        assert!(!buf.is_empty());
    }

    #[test]
    fn generate_completions_should_produce_output_for_fish() {
        let mut buf = Vec::new();
        assert!(generate_completions("fish", &mut buf).is_ok());
        assert!(!buf.is_empty());
    }

    #[test]
    fn generate_completions_should_produce_output_for_powershell() {
        let mut buf = Vec::new();
        assert!(generate_completions("powershell", &mut buf).is_ok());
        assert!(!buf.is_empty());
    }

    #[test]
    fn generate_completions_should_produce_output_for_elvish() {
        let mut buf = Vec::new();
        assert!(generate_completions("elvish", &mut buf).is_ok());
        assert!(!buf.is_empty());
    }

    #[test]
    fn generate_completions_should_return_error_for_unknown_shell() {
        let mut buf = Vec::new();
        let err = generate_completions("cmd", &mut buf).unwrap_err();
        assert!(err.contains("unknown shell"));
        assert!(err.contains("cmd"));
    }

    #[test]
    fn generate_completions_error_message_should_list_all_supported_shells() {
        let mut buf = Vec::new();
        let err = generate_completions("invalid", &mut buf).unwrap_err();
        assert!(err.contains("bash"));
        assert!(err.contains("zsh"));
        assert!(err.contains("fish"));
        assert!(err.contains("powershell"));
        assert!(err.contains("elvish"));
    }
}
