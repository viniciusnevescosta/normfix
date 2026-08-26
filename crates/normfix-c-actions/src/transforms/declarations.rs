use super::{
    BTreeSet, CActionError, CActionOptions, Edit, OnceLock, ParsedContext, Regex,
    ReportedDiagnostic, leading_whitespace, next_tab_stop, tabs_to_column, visual_width,
};

pub(super) fn format_initial_declarations(
    context: &ParsedContext,
) -> Result<Vec<Edit>, CActionError> {
    let lines = context.lines();
    let mut edits = Vec::new();
    for declaration in context
        .facts()
        .local_declarations
        .iter()
        .filter(|declaration| declaration.initial)
    {
        let start = declaration.range.start().get() as usize;
        let end = declaration.range.end().get() as usize;
        let line_number = lines.line_number_at(start);
        if lines.line_number_at(end.saturating_sub(1)) != line_number {
            continue;
        }
        let Some(line) = lines.get(line_number) else {
            continue;
        };
        let leading_end = leading_whitespace(lines.text(line));
        if line.start + leading_end != start {
            continue;
        }
        let leading = &context.source()[line.start..start];
        if leading != "\t" {
            edits.push(Edit::new(
                line.start,
                start,
                "\t",
                "INITIAL_DECLARATION_INDENT",
                "indented an initial local declaration with one tab",
                Some(line_number),
            )?);
        }
    }
    for block in &context.facts().initial_declaration_blocks {
        let Some(last) = block.declarations.last() else {
            continue;
        };
        let Some(following) = block.following_item else {
            continue;
        };
        let declaration_line = lines.line_number_at(last.end().get() as usize - 1);
        let following_line = lines.line_number_at(following.start().get() as usize);
        if following_line != declaration_line.saturating_add(1) {
            continue;
        }
        let Some(line) = lines.get(following_line) else {
            continue;
        };
        edits.push(Edit::new(
            line.start,
            line.start,
            "\n",
            "INITIAL_DECLARATION_BLANK_LINE",
            "inserted one blank line after the initial declaration block",
            Some(following_line),
        )?);
    }
    Ok(edits)
}

pub(super) fn decimal_mantissa_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^(?:\d+(?:\.\d*)?|\.\d+)$").expect("constant decimal regex is valid")
    })
}

pub(super) fn hex_mantissa_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^0[xX](?:[0-9A-Fa-f]+(?:\.[0-9A-Fa-f]*)?|\.[0-9A-Fa-f]+)$")
            .expect("constant hexadecimal regex is valid")
    })
}

#[derive(Clone, Debug)]
struct Declaration {
    line: u32,
    scope: Vec<u32>,
    text: String,
    offset: usize,
    gap_start: usize,
    gap_end: usize,
    prefix_column: u32,
    declarator_column: u32,
}

pub(super) fn align_declarations(
    context: &ParsedContext,
    diagnostics: &[ReportedDiagnostic],
    options: &CActionOptions,
) -> Result<Vec<Edit>, CActionError> {
    let targets: BTreeSet<u32> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "MISALIGNED_VAR_DECL")
        .map(|diagnostic| diagnostic.line)
        .collect();
    if targets.is_empty() && !options.format_proven_declarations {
        return Ok(Vec::new());
    }
    let groups = declaration_groups(context);
    let mut edits = Vec::new();
    for group in groups {
        if group.is_empty()
            || (!targets.is_empty()
                && !options.format_proven_declarations
                && group.iter().all(|item| !targets.contains(&item.line)))
        {
            continue;
        }
        let minimum = group
            .iter()
            .map(|item| next_tab_stop(item.prefix_column))
            .max()
            .unwrap_or(1);
        let anchor = group[0].declarator_column;
        let target = if anchor >= minimum
            && group
                .iter()
                .all(|item| tabs_to_column(item.prefix_column, anchor).is_some())
        {
            anchor
        } else {
            minimum
        };
        let mut group_edits = Vec::new();
        let mut safe = true;
        for declaration in &group {
            let Some(tabs) = tabs_to_column(declaration.prefix_column, target) else {
                safe = false;
                break;
            };
            let rebuilt = format!(
                "{}{}{}",
                &declaration.text[..declaration.gap_start],
                tabs,
                &declaration.text[declaration.gap_end..]
            );
            if visual_width(&rebuilt) > options.max_columns {
                safe = false;
                break;
            }
            group_edits.push(Edit::new(
                declaration.offset + declaration.gap_start,
                declaration.offset + declaration.gap_end,
                tabs,
                "MISALIGNED_VAR_DECL",
                "aligned a simple declaration group with tabs",
                Some(declaration.line),
            )?);
        }
        if safe {
            edits.extend(group_edits);
        }
    }
    Ok(edits)
}

fn declaration_groups(context: &ParsedContext) -> Vec<Vec<Declaration>> {
    let lines = context.lines();
    let mut scope = vec![0_u32];
    let mut next_scope = 1_u32;
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for (line_number, line, text) in lines.iter() {
        let has_protected =
            (line.start..line.content_end).any(|offset| context.lexical().is_protected(offset));
        let declaration =
            parse_declaration(text, line_number, line.start, scope.clone(), has_protected);
        let continues_group = declaration.as_ref().is_some_and(|item| {
            current.last().is_some_and(|previous: &Declaration| {
                item.scope == previous.scope && item.line == previous.line.saturating_add(1)
            })
        });
        if let Some(declaration) = declaration {
            if !continues_group && !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
            current.push(declaration);
        } else if !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }

        if !text.trim_start_matches([' ', '\t']).starts_with('#') {
            for (relative, byte) in text.bytes().enumerate() {
                if context.lexical().is_protected(line.start + relative) {
                    continue;
                }
                if byte == b'}' {
                    if scope.len() > 1 {
                        scope.pop();
                    }
                } else if byte == b'{' {
                    scope.push(next_scope);
                    next_scope = next_scope.saturating_add(1);
                }
            }
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn parse_declaration(
    text: &str,
    line: u32,
    offset: usize,
    scope: Vec<u32>,
    has_protected: bool,
) -> Option<Declaration> {
    if has_protected
        || text
            .bytes()
            .any(|byte| matches!(byte, b',' | b'(' | b')' | b':' | b'\\' | b'{' | b'}'))
        || text.bytes().filter(|byte| *byte == b';').count() != 1
        || !text.trim_end().ends_with(';')
    {
        return None;
    }
    let captures = simple_declaration_regex().captures(text)?;
    let gap = captures.name("gap")?;
    let declarator = captures.name("declarator")?;
    Some(Declaration {
        line,
        scope,
        text: text.to_owned(),
        offset,
        gap_start: gap.start(),
        gap_end: gap.end(),
        prefix_column: visual_width(&text[..gap.start()]) + 1,
        declarator_column: visual_width(&text[..declarator.start()]) + 1,
    })
}

fn simple_declaration_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?x)
            ^
            (?P<indent>\t*)
            (?P<type>
                (?:
                    (?:(?:static|extern|register|const|volatile|restrict|signed|unsigned|short|long)\x20+)*
                    (?:
                        (?:struct|union|enum)\x20+[A-Za-z_][A-Za-z0-9_]*
                        |void|char|int|float|double|_Bool|short|long|signed|unsigned
                        |va_list|size_t|ssize_t|ptrdiff_t|bool|FILE
                        |t_[A-Za-z0-9_]+|[A-Za-z_][A-Za-z0-9_]*_t|[A-Z][A-Za-z0-9_]*
                    )
                    (?:\x20+(?:const|volatile|restrict|signed|unsigned|short|long|int))*
                )
            )
            (?P<gap>[\x20\t]+)
            (?P<declarator>\*+[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*)
            (?P<arrays>(?:\[[0-9A-Z_+\-*/\x20\t]*\])*)
            (?P<initializer>[\x20\t]*=[\x20\t]*[A-Za-z0-9_+\-*/%&|^~!.<>\x20\t]+)?
            [\x20\t]*;
            $
            ",
        )
        .expect("constant simple declaration regex is valid")
    })
}

pub(super) fn is_declaration_word(word: &str) -> bool {
    matches!(
        word,
        "_Atomic"
            | "_Bool"
            | "auto"
            | "char"
            | "const"
            | "double"
            | "enum"
            | "extern"
            | "float"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "short"
            | "signed"
            | "static"
            | "struct"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
    )
}
