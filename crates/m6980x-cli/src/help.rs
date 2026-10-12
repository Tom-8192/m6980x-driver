//! Hilfetexte der Kommandozeile aus den Sprachdateien (locales/*.yml):
//!
//! * `cli.about` - Programm, `cli.cmd.<pfad>` - Befehl (z. B. `cli.cmd.dpi.set`)
//! * `cli.arg.<pfad>.<argument>` - Argument eines Befehls, sonst `cli.opt.<argument>` (gemeinsame wie `temporary`)
//! * `cli.heading.*` - Ueberschriften, die sonst clap selbst (englisch) schreibt
//!
//! Ein Test prueft, dass jeder Befehl und jedes Argument einen Text hat.

use clap::{Arg, Command};
use m6980x_core::i18n;
use m6980x_core::tr;

fn arg_text(path: &str, arg: &Arg) -> Option<String> {
    let id = arg.get_id().as_str();
    let specific = format!("cli.arg.{path}.{id}");
    [specific, format!("cli.opt.{id}")].into_iter().find(|k| i18n::has(k)).map(|k| i18n::text(&k))
}

fn cmd_key(path: &str) -> String {
    if path.is_empty() { "cli.about".into() } else { format!("cli.cmd.{path}") }
}

fn localize_cmd(cmd: Command, path: String) -> Command {
    let about = cmd_key(&path);
    let cmd = if i18n::has(&about) { cmd.about(i18n::text(&about)) } else { cmd };
    let template = format!(
        "{{before-help}}{{about-with-newline}}\n{} {{usage}}\n\n{{all-args}}{{after-help}}",
        tr!("cli.heading.usage")
    );
    let (arguments, options) = (tr!("cli.heading.arguments"), tr!("cli.heading.options"));
    let arg_path = path.clone();
    cmd.help_template(template)
        .subcommand_help_heading(tr!("cli.heading.commands"))
        .subcommand_value_name(tr!("cli.heading.command_value"))
        .mut_args(move |arg| {
            let heading = if arg.is_positional() { arguments.clone() } else { options.clone() };
            let arg = match arg_text(&arg_path, &arg) {
                Some(text) => arg.help(text),
                None => arg,
            };
            // Standardwert des Makro-Modus in der aktuellen Sprache (eingeben kann man jede)
            let arg = if arg.get_id() == "mode" { arg.default_value(i18n::name("mode.once")) } else { arg };
            // mit eigener Ueberschrift sortiert clap Positionsargumente sonst alphabetisch
            let arg = match arg.get_index() {
                Some(i) => arg.display_order(i),
                None => arg,
            };
            arg.help_heading(heading)
        })
        .mut_subcommands(move |sub| {
            let name = sub.get_name().to_string();
            let sub_path = if path.is_empty() { name } else { format!("{path}.{name}") };
            localize_cmd(sub, sub_path)
        })
}

/// Alle Texte in der aktuellen Sprache. Baut den Befehl vorher, damit clap seine eigenen --help/--version/help
/// schon angelegt und die globalen Optionen an die Unterbefehle verteilt hat.
pub fn localize(mut cmd: Command) -> Command {
    cmd.build();
    localize_cmd(cmd, String::new())
}

/// Befehle und Argumente ohne Text (fuer den Test).
#[cfg(test)]
pub fn missing(mut cmd: Command) -> Vec<String> {
    fn walk(cmd: &Command, path: &str, out: &mut Vec<String>) {
        if !i18n::has(&cmd_key(path)) {
            out.push(cmd_key(path));
        }
        for arg in cmd.get_arguments() {
            if arg_text(path, arg).is_none() {
                out.push(format!("cli.arg.{path}.{} / cli.opt.{}", arg.get_id(), arg.get_id()));
            }
        }
        // clap spiegelt unter "help" alle Befehle; dort zaehlt nur der Text von "help" selbst
        for sub in cmd.get_subcommands().filter(|_| !path.ends_with("help") && path != "help") {
            let p = if path.is_empty() { sub.get_name().to_string() } else { format!("{path}.{}", sub.get_name()) };
            walk(sub, &p, out);
        }
    }
    cmd.build();
    let mut out = Vec::new();
    walk(&cmd, "", &mut out);
    out
}
