use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use pico_args::Arguments;

use kaku_render::ThemeName;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub path: Option<PathBuf>,
    pub read_stdin: bool,
    pub plain: bool,
    pub watch: bool,
    pub toc_open: bool,
    pub syntax_highlighting: bool,
    pub theme: ThemeName,
    pub help: bool,
    pub version: bool,
}

impl CliArgs {
    pub fn parse() -> Result<Self, String> {
        let mut args = Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            return Ok(Self {
                path: None,
                read_stdin: false,
                plain: false,
                watch: false,
                toc_open: false,
                syntax_highlighting: cfg!(feature = "syntax"),
                theme: ThemeName::Auto,
                help: true,
                version: false,
            });
        }

        if args.contains(["-V", "--version"]) {
            return Ok(Self {
                path: None,
                read_stdin: false,
                plain: false,
                watch: false,
                toc_open: false,
                syntax_highlighting: cfg!(feature = "syntax"),
                theme: ThemeName::Auto,
                help: false,
                version: true,
            });
        }

        let explicit_stdin = args.contains("--stdin");
        let plain = args.contains(["-p", "--plain"]) || args.contains("--print");
        let watch = args.contains(["-w", "--watch"]);
        let toc_open = args.contains(["-t", "--toc"]);
        let syntax_highlighting = cfg!(feature = "syntax") && !args.contains("--no-syntax");

        let theme = match args.opt_value_from_str::<_, String>(["-T", "--theme"]) {
            Ok(Some(name)) => ThemeName::parse(&name).ok_or_else(|| {
                format!("unknown theme '{name}', expected auto|light|dark|minimal")
            })?,
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

        let read_stdin = explicit_stdin
            || matches!(path.as_deref(), Some(path) if path == Path::new("-"))
            || (path.is_none() && !std::io::stdin().is_terminal());

        let path = match path {
            Some(path) if path == Path::new("-") => None,
            other => other,
        };

        if !read_stdin && path.is_none() {
            return Err("expected a file path or piped stdin".to_string());
        }

        if watch && path.is_none() {
            return Err("--watch requires a file path".to_string());
        }

        Ok(Self {
            path,
            read_stdin,
            plain,
            watch,
            toc_open,
            syntax_highlighting,
            theme,
            help: false,
            version: false,
        })
    }

    pub fn usage() -> &'static str {
        "\
kaku - fast, minimal Markdown reader for terminals

USAGE:
  kaku <FILE>
  kaku [OPTIONS] <FILE>
  cat README.md | kaku
  kaku < README.md

OPTIONS:
  -p, --plain         Render once to plain stdout
  -w, --watch         Reload when the file changes
  -t, --toc           Start with the table of contents open
  -T, --theme <NAME>  auto | light | dark | minimal
  --stdin             Force reading from stdin
  --no-syntax         Disable syntax highlighting (feature builds only)
  -h, --help          Show help
  -V, --version       Show version

EXAMPLES:
  kaku README.md
  kaku -p README.md
  kaku -w README.md
  kaku -
  cat README.md | kaku
"
    }
}
