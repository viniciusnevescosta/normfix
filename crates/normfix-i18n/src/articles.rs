//! Long-form rule explanations for `normfix explain`.
//!
//! The rule identifier is language-neutral, so mapping an identifier to an
//! article happens once, and only the prose is per-locale. Each locale matches
//! exhaustively on [`ArticleKey`], which makes an untranslated article a build
//! failure rather than a silently English paragraph in a translated report.

use crate::Locale;

/// One bundled explanation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Article {
    /// One-line statement of what the rule is about.
    pub title: &'static str,
    /// Why the rule exists.
    pub why: &'static str,
    /// The concrete next step for the reader.
    pub next: &'static str,
    /// Why the tool did, or did not, act on it by itself.
    pub safety: &'static str,
}

/// Every article this build bundles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArticleKey {
    /// A function body longer than the Norm allows.
    TooManyLines,
    /// A function taking more parameters than the Norm allows.
    TooManyArgs,
    /// A function declaring more locals than the Norm allows.
    TooManyVarsFunc,
    /// A file defining more functions than the Norm allows.
    TooManyFuncs,
    /// A line wider than 80 display columns.
    LineTooLong,
    /// A non-canonical local declaration block.
    LocalDeclarationBlock,
    /// An incomplete or inconsistent header inclusion guard.
    HeaderGuard,
    /// An official checker release this build has not been verified against.
    NorminetteVersionUntested,
    /// The expected order of an include block.
    IncludeOrder,
    /// No project-root Makefile was available for preflight.
    MakefileNotFound,
    /// A root Makefile exists but was outside the scope.
    MakefileNotEvaluated,
    /// A Makefile source token that is missing or holds no code.
    MakefileSource,
    /// A header prototype with no project implementation.
    HeaderPrototypeMissing,
    /// A header prototype whose implementation is trivia only.
    HeaderPrototypeEmpty,
    /// A strict compiler preflight failure.
    CompilerPreflight,
    /// A deep static-analyzer finding.
    AnalyzerFinding,
    /// A helper that may be eligible for internal linkage.
    StaticHelperCandidate,
    /// The C parser recovered from unsupported syntax.
    ParserRecovery,
    /// A call absent from the project allowlist.
    FunctionNotAllowed,
    /// Per-function Norm headroom.
    NormBudget,
    /// A comment placed outside the accepted Norm scope.
    CommentScope,
    /// The requested analyzer is not available.
    AnalyzerUnavailable,
    /// Any other analyzer finding.
    AnalyzerGeneric,
    /// Any other strict compiler finding.
    CompilerGeneric,
    /// The fallback used when no dedicated article is bundled.
    Unknown,
}

/// Every article key, in stable order, for completeness tests.
pub const ARTICLE_KEYS: &[ArticleKey] = &[
    ArticleKey::TooManyLines,
    ArticleKey::TooManyArgs,
    ArticleKey::TooManyVarsFunc,
    ArticleKey::TooManyFuncs,
    ArticleKey::LineTooLong,
    ArticleKey::LocalDeclarationBlock,
    ArticleKey::HeaderGuard,
    ArticleKey::NorminetteVersionUntested,
    ArticleKey::IncludeOrder,
    ArticleKey::MakefileNotFound,
    ArticleKey::MakefileNotEvaluated,
    ArticleKey::MakefileSource,
    ArticleKey::HeaderPrototypeMissing,
    ArticleKey::HeaderPrototypeEmpty,
    ArticleKey::CompilerPreflight,
    ArticleKey::AnalyzerFinding,
    ArticleKey::StaticHelperCandidate,
    ArticleKey::ParserRecovery,
    ArticleKey::FunctionNotAllowed,
    ArticleKey::NormBudget,
    ArticleKey::CommentScope,
    ArticleKey::AnalyzerUnavailable,
    ArticleKey::AnalyzerGeneric,
    ArticleKey::CompilerGeneric,
    ArticleKey::Unknown,
];

/// Maps a canonical rule identifier to its article.
///
/// Identifiers are stable API tokens, so this mapping is deliberately shared by
/// every locale: a translation can change the words, never which rule an
/// explanation belongs to.
#[must_use]
pub fn article_key(canonical: &str) -> ArticleKey {
    match canonical {
        "TOO_MANY_LINES" => ArticleKey::TooManyLines,
        "TOO_MANY_ARGS" => ArticleKey::TooManyArgs,
        "TOO_MANY_VARS_FUNC" => ArticleKey::TooManyVarsFunc,
        "TOO_MANY_FUNCS" => ArticleKey::TooManyFuncs,
        "LINE_TOO_LONG" => ArticleKey::LineTooLong,
        "VAR_DECL_START_FUNC"
        | "DECL_ASSIGN_LINE"
        | "NL_AFTER_VAR_DECL"
        | "MISALIGNED_VAR_DECL"
        | "TOO_MANY_TAB"
        | "TOO_FEW_TAB" => ArticleKey::LocalDeclarationBlock,
        "HEADER_PROT_NAME" | "HEADER_PROT_NODEF" | "HEADER_PROTECTION_REVIEW" => {
            ArticleKey::HeaderGuard
        }
        "NORMINETTE_VERSION_UNTESTED" => ArticleKey::NorminetteVersionUntested,
        "INCLUDE_ORDER" | "INCLUDE_ORDER_REVIEW" => ArticleKey::IncludeOrder,
        "MAKEFILE_NOT_FOUND" => ArticleKey::MakefileNotFound,
        "MAKEFILE_NOT_EVALUATED" => ArticleKey::MakefileNotEvaluated,
        "MAKEFILE_SOURCE_NOT_FOUND" | "MISSING_MAKEFILE_SOURCE" | "MAKEFILE_SOURCE_EMPTY" => {
            ArticleKey::MakefileSource
        }
        "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING" | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED" => {
            ArticleKey::HeaderPrototypeMissing
        }
        "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY" => ArticleKey::HeaderPrototypeEmpty,
        "COMPILER_WARNING" | "COMPILER_PREFLIGHT" => ArticleKey::CompilerPreflight,
        "ANALYZER_WARNING" => ArticleKey::AnalyzerFinding,
        "STATIC_HELPER_CANDIDATE" => ArticleKey::StaticHelperCandidate,
        "C_PARSER_FAILURE" | "PARSER_RECOVERY" => ArticleKey::ParserRecovery,
        "FUNCTION_NOT_ALLOWED" => ArticleKey::FunctionNotAllowed,
        "NORM_BUDGET" | "FUNCTION_BUDGET" => ArticleKey::NormBudget,
        "WRONG_SCOPE_COMMENT" | "COMMENT_ON_INSTR" => ArticleKey::CommentScope,
        "CC_ANALYZER_UNAVAILABLE" => ArticleKey::AnalyzerUnavailable,
        // Prefix families come from the compiler itself, so the authoritative
        // message is already in the diagnostic and the article stays generic.
        other if other.starts_with("CC_ANALYZER_") => ArticleKey::AnalyzerGeneric,
        other if other.starts_with("CC_") => ArticleKey::CompilerGeneric,
        _ => ArticleKey::Unknown,
    }
}

/// Returns the article for `key` in `locale`.
#[must_use]
pub fn article(locale: Locale, key: ArticleKey) -> Article {
    match locale {
        Locale::English => english(key),
        Locale::Portuguese => portuguese(key),
        Locale::Spanish => spanish(key),
        Locale::French => french(key),
    }
}

// One arm per article: the list is long by construction, and splitting a
// locale's catalogue would only make a missing translation harder to see.
#[allow(clippy::too_many_lines)]
fn english(key: ArticleKey) -> Article {
    match key {
        ArticleKey::TooManyLines => Article {
            title: "Function body exceeds 25 lines",
            why: "The 42 Norm limits each function body to 25 physical lines so responsibilities stay small and reviewable.",
            next: "Extract one coherent responsibility. Keep live inputs to four parameters or fewer and verify that the file still contains at most five functions.",
            safety: "normfix reports this as a suggestion because choosing a function boundary changes program structure.",
        },
        ArticleKey::TooManyArgs => Article {
            title: "Function has more than four parameters",
            why: "A Norm-compliant function may receive at most four named parameters.",
            next: "Narrow the contract or group genuinely related state in an existing project type; do not create a meaningless wrapper only to hide the count.",
            safety: "Changing a public signature is an API change, so it is never applied automatically.",
        },
        ArticleKey::TooManyVarsFunc => Article {
            title: "Function declares more than five local variables",
            why: "The limit includes variables declared in the function's initial declaration block.",
            next: "Remove redundant state or extract a cohesive operation. Moving declarations alone does not reduce the count.",
            safety: "Automatic extraction would require human naming and ownership decisions.",
        },
        ArticleKey::TooManyFuncs => Article {
            title: "File defines more than five functions",
            why: "The 42 Norm limits the number of function definitions in one C source file.",
            next: "Move a cohesive group to another .c file, then update its header and the Makefile together.",
            safety: "Cross-file refactors are suggestions until project-wide linkage and build proofs succeed.",
        },
        ArticleKey::LineTooLong => Article {
            title: "Line exceeds 80 display columns",
            why: "Tabs use four-column stops and wide Unicode characters can consume more than one terminal column.",
            next: "Break at a proven comma or binary operator. Strings, comments, macros, unary operators, and evaluation order are protected barriers.",
            safety: "normfix applies only token-preserving wraps and leaves every ambiguous line for review.",
        },
        ArticleKey::LocalDeclarationBlock => Article {
            title: "Local declaration block is not canonical",
            why: "Local declarations belong at the beginning of the function or block, one declaration per line, followed by one blank line before instructions.",
            next: "Move the declaration without its assignment, then assign shortly before the first use.",
            safety: "Hoisting can change scope or lifetime, so only structurally proven moves are automatic.",
        },
        ArticleKey::HeaderGuard => Article {
            title: "Header inclusion guard is incomplete or inconsistent",
            why: "The #ifndef and #define names must match the filename-derived guard and the final #endif must protect the whole header.",
            next: "Use one outer guard and check for project-wide macro references, #undef, X-macro use, and repeated-inclusion behavior.",
            safety: "Guard edits are accepted only with a closed-project collision proof and final Norminette validation.",
        },
        ArticleKey::NorminetteVersionUntested => Article {
            title: "The official checker is a release this version has not been verified against",
            why: "normfix is verified against one exact Norminette release, because the official rule names, locations and accepted layouts are inputs to its before/after proof. The default remains usable with a different release but names the reduced compatibility assurance explicitly.",
            next: "Install the supported release when you can, or use --strict-norminette-version in pinned CI. Until then, read the diff before accepting it and report any disagreement so the supported version can move deliberately.",
            safety: "The before/after proof still compares two answers from this same checker, so a run cannot make its own official result worse. What is not guaranteed is that the native rules agree with this release.",
        },
        ArticleKey::IncludeOrder => Article {
            title: "Include block order",
            why: "The expected display order is <system headers> first, then \"project headers\", alphabetically inside each category.",
            next: "Nothing to do when a fixing run reordered the block; reorder by hand when the report kept it, which happens with --no-reorder-includes or when a comment, conditional, or macro interrupts the run of directives.",
            safety: "A block is rewritten only when every one of its lines is exactly one include directive, so no directive is ever moved across a construct that could change what a header means.",
        },
        ArticleKey::MakefileNotFound => Article {
            title: "No project-root Makefile was available for preflight",
            why: "No regular Makefile was selected or found at the project root, so build-target and source-list checks did not run. This is normal for a piscina exercise, where only .c files are expected and both a Makefile and project headers are optional.",
            next: "Ignore this when the subject expects loose .c files. Add or select the Makefile when it expects one.",
            safety: "Absence is never a hard fail and costs no score. Only the subject can say whether a Makefile is required, and normfix does not read subjects.",
        },
        ArticleKey::MakefileNotEvaluated => Article {
            title: "The project-root Makefile was outside the preflight scope",
            why: "A regular Makefile exists at the project root, but the explicit file scope did not select it, so its header, targets, recipes, and source references were not evaluated.",
            next: "Include the root Makefile explicitly, or run preflight from the project root without a partial file scope.",
            safety: "normfix reports the incomplete coverage instead of treating an existing but uninspected Makefile as evaluated.",
        },
        ArticleKey::MakefileSource => Article {
            title: "Makefile references a missing or trivia-only C source",
            why: "A literal source listed in SRCS/SRC must resolve inside the project and contain an implementation rather than only whitespace/comments.",
            next: "Implement or restore the file, or remove the exact literal token. Dynamic Make expressions are intentionally not guessed.",
            safety: "Removal is destructive and therefore requires --unsafe plus confirmation or --force.",
        },
        ArticleKey::HeaderPrototypeMissing => Article {
            title: "Header prototype has no project implementation",
            why: "A non-static prototype in a project header has no matching non-static definition in the complete lossless project C/header set. Generated code or an external library may still provide it.",
            next: "Implement the function or verify the subject and linkage. Unsafe mode removes it only when the identifier has no project use and no macro, string, conditional, attribute, or token-paste ambiguity.",
            safety: "Removing a public declaration changes the project API, so it is capability-gated, transactionally backed up, and refused on incomplete proof.",
        },
        ArticleKey::HeaderPrototypeEmpty => Article {
            title: "Header prototype resolves to a trivia-only implementation",
            why: "The project contains a matching non-static definition, but its body has no C token beyond braces, whitespace, and comments.",
            next: "Implement the required behavior or verify against the subject that an intentional no-op is valid.",
            safety: "normfix only warns: it does not remove an existing definition or its public prototype because an empty body can be intentional.",
        },
        ArticleKey::CompilerPreflight => Article {
            title: "Strict compiler preflight failed",
            why: "42 projects are expected to compile with -Wall -Wextra -Werror; -Werror promotes every emitted warning to an error.",
            next: "Follow the compiler location and message. This check does not claim to detect memory leaks.",
            safety: "Compiler diagnostics are read-only and never authorize a source rewrite.",
        },
        ArticleKey::AnalyzerFinding => Article {
            title: "Static-analyzer finding",
            why: "GCC -fanalyzer explores paths and may report leaks or invalid accesses, but it is incomplete across translation units and ownership stored in structs.",
            next: "Treat the finding as evidence to investigate, then confirm with project tests and runtime tools.",
            safety: "Analyzer output is automatic in preflight (or requested with --analyzer), informational, fail-open, and never part of the fix proof gate.",
        },
        ArticleKey::StaticHelperCandidate => Article {
            title: "Project-local helper may be eligible for static linkage",
            why: "A function used only inside its translation unit should normally have internal linkage.",
            next: "Confirm it is absent from public headers, callbacks, generated references, macros, and other translation units before adding static.",
            safety: "Linkage changes are suggestions unless a complete project graph proves them.",
        },
        ArticleKey::ParserRecovery => Article {
            title: "C parser recovered from invalid or unsupported syntax",
            why: "A missing or extra token makes the intended program ambiguous, so formatting through the recovery region could corrupt code.",
            next: "Repair the syntax at the reported location and run normfix again.",
            safety: "normfix never guesses arbitrary parentheses, braces, semicolons, or operators.",
        },
        ArticleKey::FunctionNotAllowed => Article {
            title: "Call is absent from the project allowlist",
            why: "normfix.toml declares the external functions permitted by the current 42 subject, and this recoverable direct call is not listed.",
            next: "Check the subject, then remove the call or add its exact name to [project].allowed only when it is genuinely authorized.",
            safety: "Macros, local definitions, parameters, locals, and ambiguous function-pointer calls are excluded conservatively.",
        },
        ArticleKey::NormBudget => Article {
            title: "Per-function Norm headroom",
            why: "This informational row shows current body lines, local variables, and parameters against the 25/5/4 limits.",
            next: "Keep some headroom for defense-day changes; exceeded limits also appear as dedicated warnings.",
            safety: "Budget reporting is read-only and automatic function extraction is intentionally not attempted.",
        },
        ArticleKey::CommentScope => Article {
            title: "Comment placement is outside the accepted Norm scope",
            why: "The official oracle rejected a comment at this exact location.",
            next: "Move or rewrite the comment in English, or explicitly request removal when losing the comment is acceptable.",
            safety: "Comments are only removed through the explicit opt-in path; ordinary formatting preserves them.",
        },
        ArticleKey::AnalyzerUnavailable => Article {
            title: "The requested analyzer is not available",
            why: "Preflight or --analyzer requested a deep pass, but the selected compiler ships neither GCC -fanalyzer nor the Clang analyzer, so that pass was skipped. Nothing was analyzed and nothing failed.",
            next: "Point --cc at a real GCC or Clang. Outside preflight, omit --analyzer to skip the attempt; preflight always tries the bounded analyzer. On macOS, /usr/bin/gcc is Clang under another name.",
            safety: "This is informational and fail-open: a missing analyzer never changes the exit status and never blocks a fix.",
        },
        ArticleKey::AnalyzerGeneric => Article {
            title: "Static-analyzer finding",
            why: "GCC -fanalyzer found a path worth investigating; it is not a complete proof of a leak or invalid access.",
            next: "Inspect the compiler location, reproduce the path with tests, and confirm ownership with a runtime tool when available.",
            safety: "Analyzer output is automatic in preflight (or requested with --analyzer), informational, fail-open, and never authorizes a rewrite.",
        },
        ArticleKey::CompilerGeneric => Article {
            title: "Strict compiler preflight finding",
            why: "The real project source was checked with -fsyntax-only -Wall -Wextra -Werror and the compiler reported this issue.",
            next: "Follow the compiler location and message, then run the project Makefile separately with the subject's required toolchain.",
            safety: "Compiler diagnostics are read-only and never authorize source edits.",
        },
        ArticleKey::Unknown => Article {
            title: "Rule reported by an analysis backend",
            why: "No dedicated long-form article is bundled for this identifier; the normal diagnostic includes the authoritative message, location, source, and contextual help.",
            next: "Run normfix again with --verbose, inspect the highlighted source, and apply the diagnostic's Next/help guidance.",
            safety: "An unknown explanation never enables an automatic edit. Edits still require their normal structural and oracle proofs.",
        },
    }
}

// One arm per article: the list is long by construction, and splitting a
// locale's catalogue would only make a missing translation harder to see.
#[allow(clippy::too_many_lines)]
fn portuguese(key: ArticleKey) -> Article {
    match key {
        ArticleKey::TooManyLines => Article {
            title: "O corpo da função passa de 25 linhas",
            why: "A Norm da 42 limita cada corpo de função a 25 linhas físicas para que as responsabilidades continuem pequenas e revisáveis.",
            next: "Extraia uma responsabilidade coerente. Mantenha as entradas em quatro parâmetros ou menos e verifique se o arquivo ainda tem no máximo cinco funções.",
            safety: "O normfix relata isso como sugestão porque escolher a fronteira de uma função muda a estrutura do programa.",
        },
        ArticleKey::TooManyArgs => Article {
            title: "A função tem mais de quatro parâmetros",
            why: "Uma função de acordo com a Norm pode receber no máximo quatro parâmetros nomeados.",
            next: "Estreite o contrato ou agrupe estado genuinamente relacionado em um tipo já existente do projeto; não crie um invólucro sem sentido só para esconder a contagem.",
            safety: "Mudar uma assinatura pública é uma mudança de API, então isso nunca é aplicado automaticamente.",
        },
        ArticleKey::TooManyVarsFunc => Article {
            title: "A função declara mais de cinco variáveis locais",
            why: "O limite inclui as variáveis declaradas no bloco inicial de declarações da função.",
            next: "Remova estado redundante ou extraia uma operação coesa. Só mover declarações não reduz a contagem.",
            safety: "A extração automática exigiria decisões humanas de nome e de posse.",
        },
        ArticleKey::TooManyFuncs => Article {
            title: "O arquivo define mais de cinco funções",
            why: "A Norm da 42 limita o número de definições de função em um arquivo-fonte C.",
            next: "Mova um grupo coeso para outro arquivo .c e atualize o cabeçalho dele e o Makefile junto.",
            safety: "Refatorações entre arquivos são sugestões até que as provas de linkagem e de build do projeto inteiro passem.",
        },
        ArticleKey::LineTooLong => Article {
            title: "A linha passa de 80 colunas de exibição",
            why: "Tabulações usam paradas de quatro colunas e caracteres Unicode largos podem consumir mais de uma coluna do terminal.",
            next: "Quebre em uma vírgula ou operador binário comprovado. Strings, comentários, macros, operadores unários e a ordem de avaliação são barreiras protegidas.",
            safety: "O normfix aplica apenas quebras que preservam tokens e deixa toda linha ambígua para revisão.",
        },
        ArticleKey::LocalDeclarationBlock => Article {
            title: "O bloco de declarações locais não está canônico",
            why: "As declarações locais pertencem ao início da função ou do bloco, uma por linha, seguidas de uma linha em branco antes das instruções.",
            next: "Mova a declaração sem a atribuição e atribua logo antes do primeiro uso.",
            safety: "Elevar uma declaração pode mudar escopo ou tempo de vida, então só movimentos estruturalmente comprovados são automáticos.",
        },
        ArticleKey::HeaderGuard => Article {
            title: "A guarda de inclusão do cabeçalho está incompleta ou inconsistente",
            why: "Os nomes do #ifndef e do #define precisam corresponder à guarda derivada do nome do arquivo, e o #endif final precisa proteger o cabeçalho inteiro.",
            next: "Use uma única guarda externa e verifique referências à macro no projeto inteiro, #undef, uso de X-macro e comportamento de inclusão repetida.",
            safety: "Edições de guarda só são aceitas com uma prova fechada de colisão no projeto e validação final da Norminette.",
        },
        ArticleKey::NorminetteVersionUntested => Article {
            title: "O verificador oficial é uma versão contra a qual esta versão não foi verificada",
            why: "O normfix é verificado contra uma versão exata da Norminette, porque os nomes de regra, as localizações e os layouts aceitos oficiais são entradas da sua prova antes/depois. O padrão continua utilizável com outra versão, mas nomeia explicitamente a garantia reduzida de compatibilidade.",
            next: "Instale a versão suportada quando puder, ou use --strict-norminette-version em uma CI fixada. Até lá, leia o diff antes de aceitá-lo e relate qualquer divergência para que a versão suportada avance de forma deliberada.",
            safety: "A prova antes/depois ainda compara duas respostas deste mesmo verificador, então uma execução não pode piorar o próprio resultado oficial. O que não está garantido é que as regras nativas concordem com esta versão.",
        },
        ArticleKey::IncludeOrder => Article {
            title: "Ordem do bloco de includes",
            why: "A ordem esperada é <cabeçalhos de sistema> primeiro, depois \"cabeçalhos do projeto\", em ordem alfabética dentro de cada categoria.",
            next: "Nada a fazer quando uma execução de correção reordenou o bloco; reordene à mão quando o relatório o manteve, o que acontece com --no-reorder-includes ou quando um comentário, condicional ou macro interrompe a sequência de diretivas.",
            safety: "Um bloco só é reescrito quando cada uma de suas linhas é exatamente uma diretiva de include, então nenhuma diretiva atravessa uma construção que poderia mudar o significado de um cabeçalho.",
        },
        ArticleKey::MakefileNotFound => Article {
            title: "Nenhum Makefile na raiz do projeto estava disponível para o preflight",
            why: "Nenhum Makefile regular foi selecionado ou encontrado na raiz do projeto, então as verificações de alvos de build e de lista de fontes não rodaram. Isso é normal em um exercício de piscina, onde só arquivos .c são esperados e tanto o Makefile quanto os cabeçalhos do projeto são opcionais.",
            next: "Ignore isso quando o assunto espera arquivos .c soltos. Adicione ou selecione o Makefile quando ele exigir um.",
            safety: "A ausência nunca reprova e não custa nota. Só o assunto pode dizer se um Makefile é exigido, e o normfix não lê assuntos.",
        },
        ArticleKey::MakefileNotEvaluated => Article {
            title: "O Makefile da raiz do projeto ficou fora do escopo do preflight",
            why: "Existe um Makefile regular na raiz do projeto, mas o escopo explícito de arquivos não o selecionou, então o cabeçalho, os alvos, as receitas e as referências de fonte dele não foram avaliados.",
            next: "Inclua o Makefile da raiz explicitamente, ou rode o preflight a partir da raiz do projeto sem um escopo parcial de arquivos.",
            safety: "O normfix relata a cobertura incompleta em vez de tratar como avaliado um Makefile que existe mas não foi inspecionado.",
        },
        ArticleKey::MakefileSource => Article {
            title: "O Makefile referencia uma fonte C ausente ou só com trivialidades",
            why: "Uma fonte literal listada em SRCS/SRC precisa resolver dentro do projeto e conter uma implementação, em vez de apenas espaços e comentários.",
            next: "Implemente ou restaure o arquivo, ou remova o token literal exato. Expressões dinâmicas do Make não são adivinhadas de propósito.",
            safety: "A remoção é destrutiva e por isso exige --unsafe mais confirmação, ou --force.",
        },
        ArticleKey::HeaderPrototypeMissing => Article {
            title: "O protótipo do cabeçalho não tem implementação no projeto",
            why: "Um protótipo não estático em um cabeçalho do projeto não tem definição não estática correspondente no conjunto completo e sem perdas de fontes C/cabeçalho. Código gerado ou uma biblioteca externa ainda pode fornecê-la.",
            next: "Implemente a função ou verifique o assunto e a linkagem. O modo inseguro só a remove quando o identificador não tem uso no projeto nem ambiguidade de macro, string, condicional, atributo ou colagem de tokens.",
            safety: "Remover uma declaração pública muda a API do projeto, então isso é condicionado a capacidade, tem backup transacional e é recusado com prova incompleta.",
        },
        ArticleKey::HeaderPrototypeEmpty => Article {
            title: "O protótipo do cabeçalho resolve para uma implementação só com trivialidades",
            why: "O projeto contém uma definição não estática correspondente, mas o corpo dela não tem nenhum token C além de chaves, espaços e comentários.",
            next: "Implemente o comportamento exigido ou verifique com o assunto que um no-op intencional é válido.",
            safety: "O normfix apenas avisa: ele não remove uma definição existente nem o protótipo público dela, porque um corpo vazio pode ser intencional.",
        },
        ArticleKey::CompilerPreflight => Article {
            title: "O preflight estrito do compilador falhou",
            why: "Espera-se que projetos da 42 compilem com -Wall -Wextra -Werror; o -Werror promove todo aviso emitido a erro.",
            next: "Siga a localização e a mensagem do compilador. Esta verificação não afirma detectar vazamentos de memória.",
            safety: "Diagnósticos do compilador são somente leitura e nunca autorizam uma reescrita de fonte.",
        },
        ArticleKey::AnalyzerFinding => Article {
            title: "Achado do analisador estático",
            why: "O -fanalyzer do GCC explora caminhos e pode relatar vazamentos ou acessos inválidos, mas é incompleto entre unidades de tradução e para posse guardada em structs.",
            next: "Trate o achado como evidência a investigar, e confirme com os testes do projeto e ferramentas de execução.",
            safety: "A saída do analisador é automática no preflight (ou pedida com --analyzer), informativa, falha aberta e nunca faz parte do portão de prova das correções.",
        },
        ArticleKey::StaticHelperCandidate => Article {
            title: "A função auxiliar local pode ser elegível a linkagem static",
            why: "Uma função usada apenas dentro da própria unidade de tradução normalmente deveria ter linkagem interna.",
            next: "Confirme que ela está ausente de cabeçalhos públicos, callbacks, referências geradas, macros e outras unidades de tradução antes de adicionar static.",
            safety: "Mudanças de linkagem são sugestões, a menos que um grafo completo do projeto as prove.",
        },
        ArticleKey::ParserRecovery => Article {
            title: "O analisador de C se recuperou de sintaxe inválida ou não suportada",
            why: "Um token faltando ou sobrando torna o programa pretendido ambíguo, então formatar através da região de recuperação poderia corromper o código.",
            next: "Conserte a sintaxe na localização indicada e rode o normfix de novo.",
            safety: "O normfix nunca adivinha parênteses, chaves, ponto e vírgula ou operadores arbitrários.",
        },
        ArticleKey::FunctionNotAllowed => Article {
            title: "A chamada não está na lista de funções permitidas do projeto",
            why: "O normfix.toml declara as funções externas permitidas pelo assunto atual da 42, e esta chamada direta recuperável não está listada.",
            next: "Confira o assunto e então remova a chamada, ou adicione o nome exato dela em [project].allowed apenas quando ela for realmente autorizada.",
            safety: "Macros, definições locais, parâmetros, variáveis locais e chamadas ambíguas por ponteiro de função são excluídas de forma conservadora.",
        },
        ArticleKey::NormBudget => Article {
            title: "Folga da Norm por função",
            why: "Esta linha informativa mostra as linhas de corpo, variáveis locais e parâmetros atuais contra os limites 25/5/4.",
            next: "Mantenha alguma folga para mudanças no dia da defesa; limites ultrapassados também aparecem como avisos dedicados.",
            safety: "O relatório de orçamento é somente leitura, e a extração automática de funções não é tentada de propósito.",
        },
        ArticleKey::CommentScope => Article {
            title: "A posição do comentário está fora do escopo aceito pela Norm",
            why: "O verificador oficial rejeitou um comentário exatamente nesta localização.",
            next: "Mova ou reescreva o comentário em inglês, ou peça explicitamente a remoção quando perder o comentário for aceitável.",
            safety: "Comentários só são removidos pelo caminho explícito de opt-in; a formatação comum os preserva.",
        },
        ArticleKey::AnalyzerUnavailable => Article {
            title: "O analisador solicitado não está disponível",
            why: "O preflight ou o --analyzer pediu uma passagem profunda, mas o compilador selecionado não traz nem o -fanalyzer do GCC nem o analisador do Clang, então essa passagem foi pulada. Nada foi analisado e nada falhou.",
            next: "Aponte o --cc para um GCC ou Clang de verdade. Fora do preflight, omita o --analyzer para pular a tentativa; o preflight sempre tenta o analisador limitado. No macOS, o /usr/bin/gcc é o Clang com outro nome.",
            safety: "Isso é informativo e falha aberto: um analisador ausente nunca muda o status de saída e nunca bloqueia uma correção.",
        },
        ArticleKey::AnalyzerGeneric => Article {
            title: "Achado do analisador estático",
            why: "O -fanalyzer do GCC encontrou um caminho que vale investigar; não é prova completa de um vazamento ou de um acesso inválido.",
            next: "Inspecione a localização do compilador, reproduza o caminho com testes e confirme a posse com uma ferramenta de execução quando houver.",
            safety: "A saída do analisador é automática no preflight (ou pedida com --analyzer), informativa, falha aberta e nunca autoriza uma reescrita.",
        },
        ArticleKey::CompilerGeneric => Article {
            title: "Achado do preflight estrito do compilador",
            why: "A fonte real do projeto foi verificada com -fsyntax-only -Wall -Wextra -Werror e o compilador relatou este problema.",
            next: "Siga a localização e a mensagem do compilador, e depois rode o Makefile do projeto separadamente com a toolchain exigida pelo assunto.",
            safety: "Diagnósticos do compilador são somente leitura e nunca autorizam edições de fonte.",
        },
        ArticleKey::Unknown => Article {
            title: "Regra relatada por um analisador",
            why: "Nenhum artigo dedicado está embutido para este identificador; o diagnóstico normal inclui a mensagem autoritativa, a localização, a origem e a ajuda contextual.",
            next: "Rode o normfix de novo com --verbose, inspecione a fonte destacada e aplique a orientação de Next/help do diagnóstico.",
            safety: "Uma explicação desconhecida nunca habilita uma edição automática. As edições continuam exigindo suas provas estruturais e do verificador.",
        },
    }
}

// One arm per article: the list is long by construction, and splitting a
// locale's catalogue would only make a missing translation harder to see.
#[allow(clippy::too_many_lines)]
fn spanish(key: ArticleKey) -> Article {
    match key {
        ArticleKey::TooManyLines => Article {
            title: "El cuerpo de la función supera las 25 líneas",
            why: "La Norma de 42 limita cada cuerpo de función a 25 líneas físicas para que las responsabilidades sigan siendo pequeñas y revisables.",
            next: "Extrae una responsabilidad coherente. Mantén las entradas en cuatro parámetros o menos y verifica que el archivo siga teniendo como mucho cinco funciones.",
            safety: "normfix lo informa como sugerencia porque elegir la frontera de una función cambia la estructura del programa.",
        },
        ArticleKey::TooManyArgs => Article {
            title: "La función tiene más de cuatro parámetros",
            why: "Una función conforme a la Norma puede recibir como mucho cuatro parámetros con nombre.",
            next: "Estrecha el contrato o agrupa estado genuinamente relacionado en un tipo ya existente del proyecto; no crees un envoltorio sin sentido solo para esconder la cuenta.",
            safety: "Cambiar una firma pública es un cambio de API, así que nunca se aplica automáticamente.",
        },
        ArticleKey::TooManyVarsFunc => Article {
            title: "La función declara más de cinco variables locales",
            why: "El límite incluye las variables declaradas en el bloque inicial de declaraciones de la función.",
            next: "Elimina estado redundante o extrae una operación cohesiva. Mover declaraciones por sí solo no reduce la cuenta.",
            safety: "La extracción automática exigiría decisiones humanas de nombre y de propiedad.",
        },
        ArticleKey::TooManyFuncs => Article {
            title: "El archivo define más de cinco funciones",
            why: "La Norma de 42 limita el número de definiciones de función en un archivo fuente C.",
            next: "Mueve un grupo cohesivo a otro archivo .c y actualiza su cabecera y el Makefile a la vez.",
            safety: "Las refactorizaciones entre archivos son sugerencias hasta que las pruebas de enlazado y compilación de todo el proyecto pasen.",
        },
        ArticleKey::LineTooLong => Article {
            title: "La línea supera las 80 columnas de visualización",
            why: "Las tabulaciones usan paradas de cuatro columnas y los caracteres Unicode anchos pueden consumir más de una columna del terminal.",
            next: "Corta en una coma u operador binario probado. Las cadenas, los comentarios, las macros, los operadores unarios y el orden de evaluación son barreras protegidas.",
            safety: "normfix aplica solo cortes que preservan los tokens y deja para revisión toda línea ambigua.",
        },
        ArticleKey::LocalDeclarationBlock => Article {
            title: "El bloque de declaraciones locales no es canónico",
            why: "Las declaraciones locales van al principio de la función o del bloque, una por línea, seguidas de una línea en blanco antes de las instrucciones.",
            next: "Mueve la declaración sin su asignación y asigna justo antes del primer uso.",
            safety: "Elevar una declaración puede cambiar el ámbito o el tiempo de vida, así que solo los movimientos probados estructuralmente son automáticos.",
        },
        ArticleKey::HeaderGuard => Article {
            title: "La guarda de inclusión de la cabecera está incompleta o es inconsistente",
            why: "Los nombres de #ifndef y #define deben coincidir con la guarda derivada del nombre del archivo, y el #endif final debe proteger toda la cabecera.",
            next: "Usa una única guarda exterior y comprueba referencias a la macro en todo el proyecto, #undef, uso de X-macro y comportamiento de inclusión repetida.",
            safety: "Las ediciones de guarda solo se aceptan con una prueba cerrada de colisión en el proyecto y validación final de Norminette.",
        },
        ArticleKey::NorminetteVersionUntested => Article {
            title: "El verificador oficial es una versión contra la que esta versión no se ha verificado",
            why: "normfix se verifica contra una versión exacta de Norminette, porque los nombres de regla, las ubicaciones y las disposiciones aceptadas oficiales son entradas de su prueba antes/después. El comportamiento por defecto sigue siendo utilizable con otra versión, pero nombra explícitamente la garantía reducida de compatibilidad.",
            next: "Instala la versión soportada cuando puedas, o usa --strict-norminette-version en una CI fijada. Hasta entonces, lee el diff antes de aceptarlo e informa de cualquier discrepancia para que la versión soportada avance de forma deliberada.",
            safety: "La prueba antes/después sigue comparando dos respuestas de este mismo verificador, así que una ejecución no puede empeorar su propio resultado oficial. Lo que no se garantiza es que las reglas nativas coincidan con esta versión.",
        },
        ArticleKey::IncludeOrder => Article {
            title: "Orden del bloque de includes",
            why: "El orden esperado es <cabeceras de sistema> primero, luego \"cabeceras del proyecto\", en orden alfabético dentro de cada categoría.",
            next: "Nada que hacer cuando una ejecución de corrección reordenó el bloque; reordénalo a mano cuando el informe lo mantuvo, lo que ocurre con --no-reorder-includes o cuando un comentario, un condicional o una macro interrumpe la secuencia de directivas.",
            safety: "Un bloque solo se reescribe cuando cada una de sus líneas es exactamente una directiva de include, así que ninguna directiva cruza una construcción que podría cambiar el significado de una cabecera.",
        },
        ArticleKey::MakefileNotFound => Article {
            title: "No había un Makefile en la raíz del proyecto para el preflight",
            why: "No se seleccionó ni se encontró un Makefile regular en la raíz del proyecto, así que las comprobaciones de objetivos de compilación y de lista de fuentes no se ejecutaron. Esto es normal en un ejercicio de piscina, donde solo se esperan archivos .c y tanto el Makefile como las cabeceras del proyecto son opcionales.",
            next: "Ignora esto cuando la asignatura espera archivos .c sueltos. Añade o selecciona el Makefile cuando exija uno.",
            safety: "La ausencia nunca suspende y no cuesta nota. Solo la asignatura puede decir si se exige un Makefile, y normfix no lee asignaturas.",
        },
        ArticleKey::MakefileNotEvaluated => Article {
            title: "El Makefile de la raíz quedó fuera del alcance del preflight",
            why: "Existe un Makefile regular en la raíz del proyecto, pero el alcance explícito de archivos no lo seleccionó, así que su cabecera, sus objetivos, sus recetas y sus referencias de fuentes no se evaluaron.",
            next: "Incluye el Makefile de la raíz explícitamente, o ejecuta el preflight desde la raíz del proyecto sin un alcance parcial de archivos.",
            safety: "normfix informa de la cobertura incompleta en lugar de tratar como evaluado un Makefile que existe pero no se inspeccionó.",
        },
        ArticleKey::MakefileSource => Article {
            title: "El Makefile referencia una fuente C ausente o solo con trivialidades",
            why: "Una fuente literal listada en SRCS/SRC debe resolverse dentro del proyecto y contener una implementación, no solo espacios y comentarios.",
            next: "Implementa o restaura el archivo, o elimina el token literal exacto. Las expresiones dinámicas de Make no se adivinan a propósito.",
            safety: "La eliminación es destructiva y por eso exige --unsafe más confirmación, o --force.",
        },
        ArticleKey::HeaderPrototypeMissing => Article {
            title: "El prototipo de la cabecera no tiene implementación en el proyecto",
            why: "Un prototipo no estático de una cabecera del proyecto no tiene definición no estática correspondiente en el conjunto completo y sin pérdidas de fuentes C/cabecera. Código generado o una biblioteca externa aún podría proporcionarla.",
            next: "Implementa la función o verifica la asignatura y el enlazado. El modo inseguro solo la elimina cuando el identificador no tiene uso en el proyecto ni ambigüedad de macro, cadena, condicional, atributo o pegado de tokens.",
            safety: "Eliminar una declaración pública cambia la API del proyecto, así que está condicionado a una capacidad, tiene copia transaccional y se rechaza con una prueba incompleta.",
        },
        ArticleKey::HeaderPrototypeEmpty => Article {
            title: "El prototipo de la cabecera resuelve a una implementación solo con trivialidades",
            why: "El proyecto contiene una definición no estática correspondiente, pero su cuerpo no tiene ningún token C más allá de llaves, espacios y comentarios.",
            next: "Implementa el comportamiento exigido o verifica con la asignatura que un no-op intencionado es válido.",
            safety: "normfix solo avisa: no elimina una definición existente ni su prototipo público, porque un cuerpo vacío puede ser intencionado.",
        },
        ArticleKey::CompilerPreflight => Article {
            title: "El preflight estricto del compilador falló",
            why: "Se espera que los proyectos de 42 compilen con -Wall -Wextra -Werror; -Werror convierte en error cada aviso emitido.",
            next: "Sigue la ubicación y el mensaje del compilador. Esta comprobación no afirma detectar fugas de memoria.",
            safety: "Los diagnósticos del compilador son de solo lectura y nunca autorizan una reescritura de la fuente.",
        },
        ArticleKey::AnalyzerFinding => Article {
            title: "Hallazgo del analizador estático",
            why: "El -fanalyzer de GCC explora rutas y puede informar de fugas o accesos inválidos, pero es incompleto entre unidades de traducción y para la propiedad guardada en structs.",
            next: "Trata el hallazgo como una pista que investigar, y confírmalo con las pruebas del proyecto y herramientas de ejecución.",
            safety: "La salida del analizador es automática en preflight (o se pide con --analyzer), informativa, falla abierta y nunca forma parte de la puerta de prueba de las correcciones.",
        },
        ArticleKey::StaticHelperCandidate => Article {
            title: "La función auxiliar local podría admitir enlazado static",
            why: "Una función usada solo dentro de su unidad de traducción normalmente debería tener enlazado interno.",
            next: "Confirma que está ausente de cabeceras públicas, callbacks, referencias generadas, macros y otras unidades de traducción antes de añadir static.",
            safety: "Los cambios de enlazado son sugerencias salvo que un grafo completo del proyecto los pruebe.",
        },
        ArticleKey::ParserRecovery => Article {
            title: "El analizador de C se recuperó de sintaxis inválida o no soportada",
            why: "Un token que falta o que sobra vuelve ambiguo el programa pretendido, así que formatear a través de la región de recuperación podría corromper el código.",
            next: "Repara la sintaxis en la ubicación indicada y vuelve a ejecutar normfix.",
            safety: "normfix nunca adivina paréntesis, llaves, puntos y coma u operadores arbitrarios.",
        },
        ArticleKey::FunctionNotAllowed => Article {
            title: "La llamada no está en la lista de funciones permitidas del proyecto",
            why: "normfix.toml declara las funciones externas permitidas por la asignatura actual de 42, y esta llamada directa recuperable no está listada.",
            next: "Consulta la asignatura y luego elimina la llamada, o añade su nombre exacto a [project].allowed solo cuando esté realmente autorizada.",
            safety: "Las macros, las definiciones locales, los parámetros, las variables locales y las llamadas ambiguas por puntero a función se excluyen de forma conservadora.",
        },
        ArticleKey::NormBudget => Article {
            title: "Margen de la Norma por función",
            why: "Esta fila informativa muestra las líneas de cuerpo, las variables locales y los parámetros actuales frente a los límites 25/5/4.",
            next: "Conserva algo de margen para los cambios del día de la defensa; los límites superados también aparecen como avisos dedicados.",
            safety: "El informe de presupuesto es de solo lectura, y la extracción automática de funciones no se intenta a propósito.",
        },
        ArticleKey::CommentScope => Article {
            title: "La colocación del comentario está fuera del ámbito aceptado por la Norma",
            why: "El verificador oficial rechazó un comentario exactamente en esta ubicación.",
            next: "Mueve o reescribe el comentario en inglés, o pide explícitamente su eliminación cuando perderlo sea aceptable.",
            safety: "Los comentarios solo se eliminan por la vía explícita de aceptación; el formateo corriente los conserva.",
        },
        ArticleKey::AnalyzerUnavailable => Article {
            title: "El analizador solicitado no está disponible",
            why: "Preflight o --analyzer pidió una pasada profunda, pero el compilador seleccionado no trae ni el -fanalyzer de GCC ni el analizador de Clang, así que esa pasada se omitió. No se analizó nada y nada falló.",
            next: "Apunta --cc a un GCC o Clang de verdad. Fuera de preflight, omite --analyzer para saltarte el intento; preflight siempre intenta el analizador acotado. En macOS, /usr/bin/gcc es Clang con otro nombre.",
            safety: "Esto es informativo y falla abierto: un analizador ausente nunca cambia el estado de salida ni bloquea una corrección.",
        },
        ArticleKey::AnalyzerGeneric => Article {
            title: "Hallazgo del analizador estático",
            why: "El -fanalyzer de GCC encontró una ruta que merece investigarse; no es prueba completa de una fuga ni de un acceso inválido.",
            next: "Inspecciona la ubicación del compilador, reproduce la ruta con pruebas y confirma la propiedad con una herramienta de ejecución cuando la haya.",
            safety: "La salida del analizador es automática en preflight (o se pide con --analyzer), informativa, falla abierta y nunca autoriza una reescritura.",
        },
        ArticleKey::CompilerGeneric => Article {
            title: "Hallazgo del preflight estricto del compilador",
            why: "La fuente real del proyecto se comprobó con -fsyntax-only -Wall -Wextra -Werror y el compilador informó de este problema.",
            next: "Sigue la ubicación y el mensaje del compilador, y luego ejecuta el Makefile del proyecto por separado con la cadena de herramientas que exige la asignatura.",
            safety: "Los diagnósticos del compilador son de solo lectura y nunca autorizan ediciones de la fuente.",
        },
        ArticleKey::Unknown => Article {
            title: "Regla informada por un analizador",
            why: "No hay ningún artículo dedicado incluido para este identificador; el diagnóstico normal incluye el mensaje con autoridad, la ubicación, el origen y la ayuda contextual.",
            next: "Vuelve a ejecutar normfix con --verbose, inspecciona la fuente resaltada y aplica la guía de Next/help del diagnóstico.",
            safety: "Una explicación desconocida nunca habilita una edición automática. Las ediciones siguen exigiendo sus pruebas estructurales y del verificador.",
        },
    }
}

// One arm per article: the list is long by construction, and splitting a
// locale's catalogue would only make a missing translation harder to see.
#[allow(clippy::too_many_lines)]
fn french(key: ArticleKey) -> Article {
    match key {
        ArticleKey::TooManyLines => Article {
            title: "Le corps de la fonction dépasse 25 lignes",
            why: "La Norme 42 limite chaque corps de fonction à 25 lignes physiques pour que les responsabilités restent petites et relisables.",
            next: "Extrayez une responsabilité cohérente. Gardez les entrées à quatre paramètres ou moins et vérifiez que le fichier contient toujours au plus cinq fonctions.",
            safety: "normfix le signale comme une suggestion, car choisir la frontière d'une fonction change la structure du programme.",
        },
        ArticleKey::TooManyArgs => Article {
            title: "La fonction a plus de quatre paramètres",
            why: "Une fonction conforme à la Norme peut recevoir au plus quatre paramètres nommés.",
            next: "Resserrez le contrat ou regroupez un état réellement lié dans un type existant du projet ; ne créez pas un enrobage vide de sens juste pour masquer le compte.",
            safety: "Changer une signature publique est un changement d'API, ce n'est donc jamais appliqué automatiquement.",
        },
        ArticleKey::TooManyVarsFunc => Article {
            title: "La fonction déclare plus de cinq variables locales",
            why: "La limite inclut les variables déclarées dans le bloc de déclarations initial de la fonction.",
            next: "Supprimez l'état redondant ou extrayez une opération cohérente. Déplacer des déclarations ne réduit pas le compte.",
            safety: "Une extraction automatique exigerait des décisions humaines de nommage et de propriété.",
        },
        ArticleKey::TooManyFuncs => Article {
            title: "Le fichier définit plus de cinq fonctions",
            why: "La Norme 42 limite le nombre de définitions de fonction dans un fichier source C.",
            next: "Déplacez un groupe cohérent vers un autre fichier .c, puis mettez à jour son en-tête et le Makefile ensemble.",
            safety: "Les refactorisations entre fichiers restent des suggestions tant que les preuves d'édition de liens et de compilation du projet entier n'ont pas abouti.",
        },
        ArticleKey::LineTooLong => Article {
            title: "La ligne dépasse 80 colonnes d'affichage",
            why: "Les tabulations utilisent des taquets de quatre colonnes et les caractères Unicode larges peuvent occuper plus d'une colonne du terminal.",
            next: "Coupez sur une virgule ou un opérateur binaire prouvé. Les chaînes, les commentaires, les macros, les opérateurs unaires et l'ordre d'évaluation sont des barrières protégées.",
            safety: "normfix n'applique que des coupures qui préservent les jetons et laisse toute ligne ambiguë à la relecture.",
        },
        ArticleKey::LocalDeclarationBlock => Article {
            title: "Le bloc de déclarations locales n'est pas canonique",
            why: "Les déclarations locales vont au début de la fonction ou du bloc, une par ligne, suivies d'une ligne vide avant les instructions.",
            next: "Déplacez la déclaration sans son affectation, puis affectez juste avant la première utilisation.",
            safety: "Remonter une déclaration peut changer la portée ou la durée de vie, donc seuls les déplacements prouvés structurellement sont automatiques.",
        },
        ArticleKey::HeaderGuard => Article {
            title: "La garde d'inclusion de l'en-tête est incomplète ou incohérente",
            why: "Les noms du #ifndef et du #define doivent correspondre à la garde dérivée du nom de fichier, et le #endif final doit protéger tout l'en-tête.",
            next: "Utilisez une seule garde externe et vérifiez les références à la macro dans tout le projet, les #undef, l'usage de X-macro et le comportement en inclusion répétée.",
            safety: "Les modifications de garde ne sont acceptées qu'avec une preuve fermée de collision dans le projet et une validation finale de Norminette.",
        },
        ArticleKey::NorminetteVersionUntested => Article {
            title: "Le vérificateur officiel est une version face à laquelle cette version n'a pas été vérifiée",
            why: "normfix est vérifié face à une version exacte de Norminette, car les noms de règles, les emplacements et les dispositions acceptées officiels sont des entrées de sa preuve avant/après. Le comportement par défaut reste utilisable avec une autre version, mais nomme explicitement la garantie de compatibilité réduite.",
            next: "Installez la version prise en charge quand vous le pouvez, ou utilisez --strict-norminette-version dans une CI figée. En attendant, lisez le diff avant de l'accepter et signalez tout désaccord pour que la version prise en charge évolue délibérément.",
            safety: "La preuve avant/après compare toujours deux réponses de ce même vérificateur, donc une exécution ne peut pas dégrader son propre résultat officiel. Ce qui n'est pas garanti, c'est que les règles natives soient d'accord avec cette version.",
        },
        ArticleKey::IncludeOrder => Article {
            title: "Ordre du bloc d'includes",
            why: "L'ordre attendu est <en-têtes système> d'abord, puis \"en-têtes du projet\", par ordre alphabétique dans chaque catégorie.",
            next: "Rien à faire quand une exécution de correction a réordonné le bloc ; réordonnez à la main quand le rapport l'a laissé tel quel, ce qui arrive avec --no-reorder-includes ou quand un commentaire, une condition ou une macro interrompt la suite de directives.",
            safety: "Un bloc n'est réécrit que lorsque chacune de ses lignes est exactement une directive d'include, si bien qu'aucune directive ne traverse une construction qui pourrait changer le sens d'un en-tête.",
        },
        ArticleKey::MakefileNotFound => Article {
            title: "Aucun Makefile à la racine du projet n'était disponible pour le preflight",
            why: "Aucun Makefile ordinaire n'a été sélectionné ni trouvé à la racine du projet, donc les vérifications de cibles de compilation et de liste de sources n'ont pas eu lieu. C'est normal pour un exercice de piscine, où seuls des fichiers .c sont attendus et où le Makefile comme les en-têtes du projet sont facultatifs.",
            next: "Ignorez ceci quand le sujet attend des fichiers .c isolés. Ajoutez ou sélectionnez le Makefile quand il en exige un.",
            safety: "Une absence n'échoue jamais et ne coûte aucun point. Seul le sujet peut dire si un Makefile est exigé, et normfix ne lit pas les sujets.",
        },
        ArticleKey::MakefileNotEvaluated => Article {
            title: "Le Makefile de la racine était hors de la portée du preflight",
            why: "Un Makefile ordinaire existe à la racine du projet, mais la portée explicite de fichiers ne l'a pas sélectionné, donc son en-tête, ses cibles, ses recettes et ses références de sources n'ont pas été évalués.",
            next: "Incluez explicitement le Makefile de la racine, ou lancez le preflight depuis la racine du projet sans portée partielle de fichiers.",
            safety: "normfix signale la couverture incomplète plutôt que de traiter comme évalué un Makefile qui existe mais n'a pas été inspecté.",
        },
        ArticleKey::MakefileSource => Article {
            title: "Le Makefile référence une source C absente ou sans code",
            why: "Une source littérale listée dans SRCS/SRC doit se résoudre dans le projet et contenir une implémentation, pas seulement des espaces et des commentaires.",
            next: "Implémentez ou restaurez le fichier, ou retirez le jeton littéral exact. Les expressions dynamiques de Make ne sont volontairement pas devinées.",
            safety: "La suppression est destructive et exige donc --unsafe plus une confirmation, ou --force.",
        },
        ArticleKey::HeaderPrototypeMissing => Article {
            title: "Le prototype d'en-tête n'a pas d'implémentation dans le projet",
            why: "Un prototype non statique d'un en-tête du projet n'a pas de définition non statique correspondante dans l'ensemble complet et sans perte des sources C/en-têtes. Du code généré ou une bibliothèque externe pourrait quand même la fournir.",
            next: "Implémentez la fonction ou vérifiez le sujet et l'édition de liens. Le mode non sûr ne la retire que lorsque l'identifiant n'a aucun usage dans le projet ni ambiguïté de macro, de chaîne, de condition, d'attribut ou de collage de jetons.",
            safety: "Retirer une déclaration publique change l'API du projet : c'est donc conditionné à une capacité, sauvegardé de façon transactionnelle, et refusé sur preuve incomplète.",
        },
        ArticleKey::HeaderPrototypeEmpty => Article {
            title: "Le prototype d'en-tête aboutit à une implémentation sans code",
            why: "Le projet contient une définition non statique correspondante, mais son corps ne contient aucun jeton C au-delà des accolades, des espaces et des commentaires.",
            next: "Implémentez le comportement attendu ou vérifiez auprès du sujet qu'un no-op intentionnel est valide.",
            safety: "normfix se contente d'avertir : il ne retire ni une définition existante ni son prototype public, car un corps vide peut être intentionnel.",
        },
        ArticleKey::CompilerPreflight => Article {
            title: "Le preflight strict du compilateur a échoué",
            why: "Les projets 42 sont censés compiler avec -Wall -Wextra -Werror ; -Werror transforme chaque avertissement émis en erreur.",
            next: "Suivez l'emplacement et le message du compilateur. Cette vérification ne prétend pas détecter les fuites de mémoire.",
            safety: "Les diagnostics du compilateur sont en lecture seule et n'autorisent jamais une réécriture de la source.",
        },
        ArticleKey::AnalyzerFinding => Article {
            title: "Constat de l'analyseur statique",
            why: "Le -fanalyzer de GCC explore des chemins et peut signaler des fuites ou des accès invalides, mais il est incomplet entre unités de traduction et pour une propriété stockée dans des structures.",
            next: "Traitez le constat comme un indice à instruire, puis confirmez-le avec les tests du projet et des outils d'exécution.",
            safety: "La sortie de l'analyseur est automatique dans preflight (ou demandée avec --analyzer), informative, en échec ouvert, et ne fait jamais partie de la porte de preuve des corrections.",
        },
        ArticleKey::StaticHelperCandidate => Article {
            title: "La fonction auxiliaire locale pourrait relever d'une liaison static",
            why: "Une fonction utilisée uniquement dans sa propre unité de traduction devrait normalement avoir une liaison interne.",
            next: "Vérifiez qu'elle est absente des en-têtes publics, des callbacks, des références générées, des macros et des autres unités de traduction avant d'ajouter static.",
            safety: "Les changements de liaison restent des suggestions tant qu'un graphe complet du projet ne les prouve pas.",
        },
        ArticleKey::ParserRecovery => Article {
            title: "L'analyseur C s'est rétabli après une syntaxe invalide ou non prise en charge",
            why: "Un jeton manquant ou en trop rend le programme voulu ambigu, donc formater à travers la zone de récupération pourrait corrompre le code.",
            next: "Réparez la syntaxe à l'emplacement signalé et relancez normfix.",
            safety: "normfix ne devine jamais de parenthèses, d'accolades, de points-virgules ou d'opérateurs arbitraires.",
        },
        ArticleKey::FunctionNotAllowed => Article {
            title: "L'appel est absent de la liste des fonctions autorisées du projet",
            why: "normfix.toml déclare les fonctions externes autorisées par le sujet 42 en cours, et cet appel direct récupérable n'y figure pas.",
            next: "Vérifiez le sujet, puis retirez l'appel ou ajoutez son nom exact à [project].allowed uniquement s'il est réellement autorisé.",
            safety: "Les macros, les définitions locales, les paramètres, les variables locales et les appels ambigus par pointeur de fonction sont exclus de façon conservatrice.",
        },
        ArticleKey::NormBudget => Article {
            title: "Marge de la Norme par fonction",
            why: "Cette ligne informative montre les lignes de corps, les variables locales et les paramètres actuels face aux limites 25/5/4.",
            next: "Gardez de la marge pour les changements du jour de la soutenance ; les limites dépassées apparaissent aussi comme des avertissements dédiés.",
            safety: "Le rapport de budget est en lecture seule, et l'extraction automatique de fonctions n'est volontairement pas tentée.",
        },
        ArticleKey::CommentScope => Article {
            title: "Le placement du commentaire sort de la portée acceptée par la Norme",
            why: "Le vérificateur officiel a rejeté un commentaire exactement à cet emplacement.",
            next: "Déplacez ou réécrivez le commentaire en anglais, ou demandez explicitement sa suppression quand le perdre est acceptable.",
            safety: "Les commentaires ne sont supprimés que par la voie explicite d'acceptation ; la mise en forme ordinaire les conserve.",
        },
        ArticleKey::AnalyzerUnavailable => Article {
            title: "L'analyseur demandé n'est pas disponible",
            why: "Preflight ou --analyzer a demandé une passe profonde, mais le compilateur choisi ne fournit ni le -fanalyzer de GCC ni l'analyseur de Clang, donc cette passe a été sautée. Rien n'a été analysé et rien n'a échoué.",
            next: "Pointez --cc vers un vrai GCC ou Clang. Hors preflight, omettez --analyzer pour éviter la tentative ; preflight tente toujours l'analyseur borné. Sur macOS, /usr/bin/gcc est Clang sous un autre nom.",
            safety: "C'est informatif et en échec ouvert : un analyseur absent ne change jamais le code de sortie et ne bloque jamais une correction.",
        },
        ArticleKey::AnalyzerGeneric => Article {
            title: "Constat de l'analyseur statique",
            why: "Le -fanalyzer de GCC a trouvé un chemin qui mérite d'être instruit ; ce n'est pas une preuve complète d'une fuite ou d'un accès invalide.",
            next: "Inspectez l'emplacement donné par le compilateur, reproduisez le chemin avec des tests, et confirmez la propriété avec un outil d'exécution quand il y en a un.",
            safety: "La sortie de l'analyseur est automatique dans preflight (ou demandée avec --analyzer), informative, en échec ouvert, et n'autorise jamais une réécriture.",
        },
        ArticleKey::CompilerGeneric => Article {
            title: "Constat du preflight strict du compilateur",
            why: "La vraie source du projet a été vérifiée avec -fsyntax-only -Wall -Wextra -Werror et le compilateur a signalé ce problème.",
            next: "Suivez l'emplacement et le message du compilateur, puis lancez le Makefile du projet séparément avec la chaîne d'outils exigée par le sujet.",
            safety: "Les diagnostics du compilateur sont en lecture seule et n'autorisent jamais de modification de la source.",
        },
        ArticleKey::Unknown => Article {
            title: "Règle signalée par un analyseur",
            why: "Aucun article dédié n'est fourni pour cet identifiant ; le diagnostic normal contient le message qui fait autorité, l'emplacement, l'origine et l'aide contextuelle.",
            next: "Relancez normfix avec --verbose, inspectez la source mise en évidence, et appliquez les conseils Next/help du diagnostic.",
            safety: "Une explication inconnue n'active jamais une modification automatique. Les modifications exigent toujours leurs preuves structurelles et celles du vérificateur.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{ARTICLE_KEYS, ArticleKey, article, article_key};
    use crate::{Locale, PUBLISHED};

    #[test]
    fn no_locale_leaves_an_article_field_empty() {
        for locale in PUBLISHED {
            for key in ARTICLE_KEYS {
                let entry = article(*locale, *key);
                for (field, value) in [
                    ("title", entry.title),
                    ("why", entry.why),
                    ("next", entry.next),
                    ("safety", entry.safety),
                ] {
                    assert!(
                        !value.trim().is_empty(),
                        "{}: {key:?}.{field} is empty",
                        locale.code()
                    );
                }
            }
        }
    }

    #[test]
    fn a_translated_article_is_actually_translated() {
        for locale in PUBLISHED.iter().filter(|l| **l != Locale::English) {
            for key in ARTICLE_KEYS {
                let english = article(Locale::English, *key);
                let translated = article(*locale, *key);
                assert_ne!(
                    english.why,
                    translated.why,
                    "{}: {key:?} still carries the English explanation",
                    locale.code()
                );
            }
        }
    }

    #[test]
    fn rule_identifiers_map_to_their_article_in_every_language() {
        // The identifier is a stable API token, so the mapping must not depend
        // on the reader's language.
        assert_eq!(article_key("TOO_MANY_LINES"), ArticleKey::TooManyLines);
        assert_eq!(
            article_key("TOO_FEW_TAB"),
            ArticleKey::LocalDeclarationBlock
        );
        assert_eq!(
            article_key("CC_ANALYZER_MALLOC_LEAK"),
            ArticleKey::AnalyzerGeneric
        );
        assert_eq!(
            article_key("CC_UNUSED_VARIABLE"),
            ArticleKey::CompilerGeneric
        );
        assert_eq!(
            article_key("CC_ANALYZER_UNAVAILABLE"),
            ArticleKey::AnalyzerUnavailable
        );
        assert_eq!(article_key("NOT_A_REAL_RULE"), ArticleKey::Unknown);
    }
}
