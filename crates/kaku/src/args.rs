use std::path::PathBuf;

use pico_args::Arguments;

use kaku_render::ThemeName;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub path: Option<PathBuf>,
    pub read_stdin: bool,
    pub print: bool,
    pub watch: bool,
    pub toc_open: bool,
    pub syntax_highlighting: bool,
    pub theme: ThemeName,
    pub help: bool,
}

impl CliArgs {
    pub fn parse() -> Result<Self, String> {
        let mut args = Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            return Ok(Self {
                path: None,
                read_stdin: false,
                print: false,
                watch: false,
                toc_open: false,
                syntax_highlighting: true,
                theme: ThemeName::Auto,
                help: true,
            });
        }

        let read_stdin = args.contains("--stdin");
        let print = args.contains("--print");
        let watch = args.contains("--watch");
        let toc_open = args.contains("--toc");
        let syntax_highlighting = !args.contains("--no-syntax");

        let theme = match args.opt_value_from_str::<_, String>("--theme") {
            Ok(Some(name)) => ThemeName::parse(&name)
                .ok_or_else(|| format!("unknown theme '{name}', expected auto|light|dark|ansi"))?,
            Ok(None) => ThemeName::Auto,
            Err(error) => return Err(error.to_string()),
        };

        let path = args
            .opt_free_from_os_str::<PathBuf, _>(|value| -> Result<PathBuf, &'static str> {
                Ok(PathBuf::from(value))
            })
            .map_err(|error| error.to_string())?;

        let remaining = args.finish();
        if !remaining.is_empty() {
            return Err(format!("unexpected argument {:?}", remaining[0]));
        }

        if !read_stdin && path.is_none() {
            return Err("expected a file path or --stdin".to_string());
        }

        Ok(Self {
            path,
            read_stdin,
            print,
            watch,
            toc_open,
            syntax_highlighting,
            theme,
            help: false,
        })
    }

    pub fn usage() -> &'static str {
        "\
kaku - fast terminal Markdown viewer

USAGE:
  kaku [OPTIONS] <FILE>
  cat README.md | kaku --stdin

OPTIONS:
  --stdin         Read Markdown from stdin
  --print         Render once to stdout instead of starting the pager
  --watch         Reload on file changes
  --toc           Start with the TOC panel open
  --theme <NAME>  auto | light | dark | ansi
  --no-syntax     Disable syntax highlighting for code blocks
  -h, --help      Show this help message
"
    }
}
