//! Command-line help texts from the language files (locales/*.yml):
//!
//! * `cli.about` - program, `cli.cmd.<path>` - command (e.g. `cli.cmd.dpi.set`)
//! * `cli.arg.<path>.<argument>` - argument of a command, otherwise `cli.opt.<argument>` (shared ones like `temporary`)
//! * `cli.heading.*` - headings that clap would otherwise write itself (in English)
//!
//! A test checks that every command and every argument has a text.

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
            // default of the macro mode in the current language (any language can be typed)
            let arg = if arg.get_id() == "mode" { arg.default_value(i18n::name("mode.once")) } else { arg };
            // with a custom heading, clap would otherwise sort positional arguments alphabetically
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

/// All texts in the current language. Builds the command first so that clap has already created its own
/// --help/--version/help and propagated the global options to the subcommands.
pub fn localize(mut cmd: Command) -> Command {
    cmd.build();
    localize_cmd(cmd, String::new())
}

/// Commands and arguments without a text (for the test).
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
        // clap mirrors all commands below "help"; only the text of "help" itself counts there
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
