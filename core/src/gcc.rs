//! Structured parsing of GCC/Clang stderr into [`Diagnostic`]s. Pure and
//! toolchain-independent, so it is fully testable without a compiler installed.

use crate::oracle::Diagnostic;

/// Parse compiler stderr. Recognizes `file:line:col: severity: message` diagnostics
/// and linker `undefined reference to \`sym'` errors.
pub fn parse_gcc_stderr(stderr: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    for raw_line in stderr.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Linker: undefined reference to `symbol'
        if let Some(pos) = line.find("undefined reference to ") {
            let rest = &line[pos + "undefined reference to ".len()..];
            let sym = rest.trim_matches(|c| c == '`' || c == '\'' || c == '"' || c == ' ');
            out.push(Diagnostic {
                severity: "error".to_string(),
                message: format!("undefined reference to {}", sym),
                file: None,
                line: None,
                col: None,
                raw: raw_line.to_string(),
            });
            continue;
        }

        // Compiler diagnostics. Check "fatal error" before "error" (substring).
        let markers: [(&str, &str); 4] = [
            (": fatal error: ", "error"),
            (": error: ", "error"),
            (": warning: ", "warning"),
            (": note: ", "note"),
        ];
        for (marker, sev) in markers {
            if let Some(pos) = line.find(marker) {
                let loc = &line[..pos];
                let msg = &line[pos + marker.len()..];
                let (file, lineno, col) = parse_location(loc);
                out.push(Diagnostic {
                    severity: sev.to_string(),
                    message: msg.trim().to_string(),
                    file,
                    line: lineno,
                    col,
                    raw: raw_line.to_string(),
                });
                break;
            }
        }
    }

    out
}

/// Parse a `file:line:col` (or `file:line`, or `file`) prefix. Splits from the right
/// so Windows drive-letter paths (`C:\a\b.c:12:5`) survive intact.
fn parse_location(loc: &str) -> (Option<String>, Option<u32>, Option<u32>) {
    let mut it = loc.rsplitn(3, ':');
    let a = it.next();
    let b = it.next();
    let c = it.next();
    match (a, b, c) {
        (Some(a), Some(b), Some(c)) => {
            if let (Ok(col), Ok(lineno)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                (Some(c.to_string()), Some(lineno), Some(col))
            } else {
                (Some(loc.to_string()), None, None)
            }
        }
        (Some(a), Some(b), None) => {
            if let Ok(lineno) = a.trim().parse::<u32>() {
                (Some(b.to_string()), Some(lineno), None)
            } else {
                (Some(loc.to_string()), None, None)
            }
        }
        _ => (Some(loc.to_string()), None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_error_with_col() {
        let d = parse_gcc_stderr("main.c:12:5: error: expected ';' before '}' token");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].severity, "error");
        assert_eq!(d[0].file.as_deref(), Some("main.c"));
        assert_eq!(d[0].line, Some(12));
        assert_eq!(d[0].col, Some(5));
    }

    #[test]
    fn parses_warning_note_and_linker() {
        let s = "foo.c:3:1: warning: unused variable 'x'\n\
                 foo.c:4:2: note: here\n\
                 /usr/bin/ld: main.o: in function `main': undefined reference to `uart_puts'";
        let d = parse_gcc_stderr(s);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].severity, "warning");
        assert_eq!(d[1].severity, "note");
        assert_eq!(d[2].severity, "error");
        assert!(d[2].message.contains("uart_puts"));
    }

    #[test]
    fn keeps_windows_path_and_parses_line_col() {
        let d = parse_gcc_stderr(r"C:\proj\main.c:7:9: error: oops");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].file.as_deref(), Some(r"C:\proj\main.c"));
        assert_eq!(d[0].line, Some(7));
        assert_eq!(d[0].col, Some(9));
    }

    #[test]
    fn fatal_error_maps_to_error() {
        let d = parse_gcc_stderr("main.c:1:10: fatal error: stm32.h: No such file or directory");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].severity, "error");
    }
}
