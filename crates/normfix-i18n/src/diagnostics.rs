//! Diagnostic text in the reader's language.
//!
//! Only diagnostics this project authors appear here. A finding from the
//! official Norminette or the C compiler is that tool's own output, so it is
//! shown exactly as produced: translating it would make the report disagree
//! with what running the tool directly prints.
//!
//! Each locale matches exhaustively on [`DiagnosticKey`], so an untranslated
//! diagnostic is a build failure rather than an English sentence inside a
//! translated report.

use crate::Locale;

/// One diagnostic's text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticText {
    /// Summary line. May carry `{placeholder}` names.
    pub message: &'static str,
    /// Context lines, in a stable order.
    pub notes: &'static [&'static str],
    /// Concrete next step.
    pub help: &'static str,
}

/// Every diagnostic this build translates.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticKey {
    /// The project root has no Makefile.
    MakefileNotFound,
    /// A root Makefile exists but was outside the scope.
    MakefileNotEvaluated,
    /// The manual checks preflight will not perform.
    PreflightManualSteps,
    /// No normfix.toml allowlist exists.
    FunctionPolicyNotConfigured,
    /// A README is present and cannot be judged automatically.
    ReadmeCriteriaReview,
    /// A Makefile source token that does not exist.
    MakefileSourceNotFound,
    /// A Makefile source that holds no code.
    MakefileSourceEmpty,
    /// A header prototype with no implementation.
    HeaderPrototypeMissing,
    /// A header prototype whose implementation is trivia only.
    HeaderPrototypeEmpty,
    /// A reported VLA whose bound is a proven constant.
    VlaCompatFalsePositive,
    /// A README reprinted as canonical `CommonMark`.
    MarkdownCanonicalFormat,
}

/// Every diagnostic key, in stable order, for completeness tests.
pub const DIAGNOSTIC_KEYS: &[DiagnosticKey] = &[
    DiagnosticKey::MakefileNotFound,
    DiagnosticKey::MakefileNotEvaluated,
    DiagnosticKey::PreflightManualSteps,
    DiagnosticKey::FunctionPolicyNotConfigured,
    DiagnosticKey::ReadmeCriteriaReview,
    DiagnosticKey::MakefileSourceNotFound,
    DiagnosticKey::MakefileSourceEmpty,
    DiagnosticKey::HeaderPrototypeMissing,
    DiagnosticKey::HeaderPrototypeEmpty,
    DiagnosticKey::VlaCompatFalsePositive,
    DiagnosticKey::MarkdownCanonicalFormat,
];

/// Returns the text for `key` in `locale`.
#[must_use]
pub fn diagnostic_text(locale: Locale, key: DiagnosticKey) -> DiagnosticText {
    match locale {
        Locale::English => english(key),
        Locale::Portuguese => portuguese(key),
        Locale::Spanish => spanish(key),
        Locale::French => french(key),
    }
}

// One arm per diagnostic: a long match is the price of making a missing
// translation impossible to merge.
#[allow(clippy::too_many_lines)]
fn english(key: DiagnosticKey) -> DiagnosticText {
    match key {
        DiagnosticKey::MakefileNotFound => DiagnosticText {
            message: "No regular Makefile was selected or found at the project root, so build-target and source-list checks did not run.",
            notes: &[
                "This is normal for a piscina exercise, where only .c files are expected: a Makefile and project headers are both optional.",
                "Absence is never a hard fail. Only the subject can say whether a Makefile is required, and normfix does not read subjects.",
            ],
            help: "Ignore this when the subject expects loose .c files; add or select the Makefile when it expects one.",
        },
        DiagnosticKey::MakefileNotEvaluated => DiagnosticText {
            message: "A regular Makefile exists at the project root but was not selected, so preflight did not evaluate it.",
            notes: &[
                "Its header, targets, recipes, and source references are absent from this report.",
            ],
            help: "Include the root Makefile explicitly, or run preflight from the project root without a partial file scope.",
        },
        DiagnosticKey::PreflightManualSteps => DiagnosticText {
            message: "Preflight does not execute project recipes, binaries, interactive tests, or runtime leak tools.",
            notes: &[
                "Run the subject's required make/relink sequence and functional tests in the evaluator environment.",
            ],
            help: "Complete the subject-specific manual checks shown in the evaluation sheet before defense.",
        },
        DiagnosticKey::FunctionPolicyNotConfigured => DiagnosticText {
            message: "Authorized-function checking is unavailable because this project has no normfix.toml allowlist.",
            notes: &[],
            help: "Create normfix.toml from the subject's exact authorized-function list before relying on preflight.",
        },
        DiagnosticKey::ReadmeCriteriaReview => DiagnosticText {
            message: "A README is present, but its subject-specific 42 evaluation criteria cannot be proven automatically.",
            notes: &[
                "README absence is not a normfix preflight failure; this advisory exists only when a README was discovered.",
            ],
            help: "Compare it with the current subject and evaluation sheet.",
        },
        DiagnosticKey::MakefileSourceNotFound => DiagnosticText {
            message: "The literal Makefile source `{source}` does not exist below the project root.",
            notes: &[
                "Only a wholly literal SRC/SRCS-style assignment was inspected; Make recipes and expansions were never executed.",
            ],
            help: "Create/correct the source path, or use the explicitly authorized unsafe removal mode to remove this exact stale token.",
        },
        DiagnosticKey::MakefileSourceEmpty => DiagnosticText {
            message: "The literal Makefile source `{source}` exists but contains no C token beyond whitespace and comments.",
            notes: &[
                "An empty source still links, so the build can succeed while the feature it names does not exist.",
            ],
            help: "Implement the file, or use the explicitly authorized unsafe removal mode to remove this exact token.",
        },
        DiagnosticKey::HeaderPrototypeMissing => DiagnosticText {
            message: "Header prototype `{name}` has no implementation in the project source set.",
            notes: &[
                "The complete lossless project source set contains no non-static definition with this identifier. Generated sources and external libraries are not inferred.",
            ],
            help: "Implement the declared function, or run explicitly authorized unsafe mode only if this project-local API is intentionally unused.",
        },
        DiagnosticKey::HeaderPrototypeEmpty => DiagnosticText {
            message: "Header prototype `{name}` resolves to an implementation with no C token beyond braces, whitespace, and comments.",
            notes: &["An intentional no-op can be valid, so normfix only reports it."],
            help: "Implement the required behavior, or verify against the subject that an empty body is intended.",
        },
        DiagnosticKey::VlaCompatFalsePositive => DiagnosticText {
            message: "Norminette reported a VLA, but `{name}` resolves to the integer constant {value}.",
            notes: &[
                "The bound is a compile-time constant, so this is a compatibility false positive rather than a variable-length array.",
            ],
            help: "Nothing to do; the array is valid C and valid under the Norm.",
        },
        DiagnosticKey::MarkdownCanonicalFormat => DiagnosticText {
            message: "The README was reprinted as canonical CommonMark.",
            notes: &[],
            help: "Use --check or --diff to preview the reprint, or --no-format-markdown to keep the document byte-for-byte.",
        },
    }
}

// One arm per diagnostic: a long match is the price of making a missing
// translation impossible to merge.
#[allow(clippy::too_many_lines)]
fn portuguese(key: DiagnosticKey) -> DiagnosticText {
    match key {
        DiagnosticKey::MakefileNotFound => DiagnosticText {
            message: "Nenhum Makefile regular foi selecionado ou encontrado na raiz do projeto, então as verificações de alvos de build e de lista de fontes não rodaram.",
            notes: &[
                "Isso é normal em um exercício de piscina, onde só arquivos .c são esperados: o Makefile e os cabeçalhos do projeto são ambos opcionais.",
                "A ausência nunca reprova. Só o assunto pode dizer se um Makefile é exigido, e o normfix não lê assuntos.",
            ],
            help: "Ignore isso quando o assunto espera arquivos .c soltos; adicione ou selecione o Makefile quando ele exigir um.",
        },
        DiagnosticKey::MakefileNotEvaluated => DiagnosticText {
            message: "Existe um Makefile regular na raiz do projeto, mas ele não foi selecionado, então o preflight não o avaliou.",
            notes: &[
                "O cabeçalho, os alvos, as receitas e as referências de fonte dele estão ausentes deste relatório.",
            ],
            help: "Inclua o Makefile da raiz explicitamente, ou rode o preflight a partir da raiz do projeto sem um escopo parcial de arquivos.",
        },
        DiagnosticKey::PreflightManualSteps => DiagnosticText {
            message: "O preflight não executa receitas do projeto, binários, testes interativos nem ferramentas de vazamento em tempo de execução.",
            notes: &[
                "Rode a sequência de make/relink exigida pelo assunto e os testes funcionais no ambiente do avaliador.",
            ],
            help: "Complete as verificações manuais específicas do assunto mostradas na ficha de avaliação antes da defesa.",
        },
        DiagnosticKey::FunctionPolicyNotConfigured => DiagnosticText {
            message: "A verificação de funções autorizadas está indisponível porque este projeto não tem uma lista permitida em normfix.toml.",
            notes: &[],
            help: "Crie o normfix.toml a partir da lista exata de funções autorizadas do assunto antes de confiar no preflight.",
        },
        DiagnosticKey::ReadmeCriteriaReview => DiagnosticText {
            message: "Existe um README, mas os critérios de avaliação da 42 específicos do assunto não podem ser comprovados automaticamente.",
            notes: &[
                "A ausência de README não reprova o preflight do normfix; este aviso só existe quando um README foi encontrado.",
            ],
            help: "Compare-o com a ficha de assunto e de avaliação atual.",
        },
        DiagnosticKey::MakefileSourceNotFound => DiagnosticText {
            message: "A fonte literal `{source}` do Makefile não existe abaixo da raiz do projeto.",
            notes: &[
                "Só uma atribuição totalmente literal no estilo SRC/SRCS foi inspecionada; receitas e expansões do Make nunca foram executadas.",
            ],
            help: "Crie ou corrija o caminho da fonte, ou use o modo de remoção insegura explicitamente autorizado para remover exatamente este token obsoleto.",
        },
        DiagnosticKey::MakefileSourceEmpty => DiagnosticText {
            message: "A fonte literal `{source}` do Makefile existe, mas não contém nenhum token C além de espaços e comentários.",
            notes: &[
                "Uma fonte vazia ainda linka, então o build pode ter sucesso enquanto a funcionalidade que ela nomeia não existe.",
            ],
            help: "Implemente o arquivo, ou use o modo de remoção insegura explicitamente autorizado para remover exatamente este token.",
        },
        DiagnosticKey::HeaderPrototypeMissing => DiagnosticText {
            message: "O protótipo `{name}` do cabeçalho não tem implementação no conjunto de fontes do projeto.",
            notes: &[
                "O conjunto completo e sem perdas de fontes do projeto não contém nenhuma definição não estática com este identificador. Fontes geradas e bibliotecas externas não são inferidas.",
            ],
            help: "Implemente a função declarada, ou use o modo inseguro explicitamente autorizado apenas se esta API local do projeto estiver intencionalmente sem uso.",
        },
        DiagnosticKey::HeaderPrototypeEmpty => DiagnosticText {
            message: "O protótipo `{name}` do cabeçalho resolve para uma implementação sem nenhum token C além de chaves, espaços e comentários.",
            notes: &["Um no-op intencional pode ser válido, então o normfix apenas relata."],
            help: "Implemente o comportamento exigido, ou verifique com o assunto que um corpo vazio é intencional.",
        },
        DiagnosticKey::VlaCompatFalsePositive => DiagnosticText {
            message: "A Norminette relatou um VLA, mas `{name}` resolve para a constante inteira {value}.",
            notes: &[
                "O limite é uma constante de tempo de compilação, então isso é um falso positivo de compatibilidade, e não um array de comprimento variável.",
            ],
            help: "Nada a fazer; o array é C válido e válido segundo a Norm.",
        },
        DiagnosticKey::MarkdownCanonicalFormat => DiagnosticText {
            message: "O README foi reimpresso como CommonMark canônico.",
            notes: &[],
            help: "Use --check ou --diff para pré-visualizar a reimpressão, ou --no-format-markdown para manter o documento byte a byte.",
        },
    }
}

// One arm per diagnostic: a long match is the price of making a missing
// translation impossible to merge.
#[allow(clippy::too_many_lines)]
fn spanish(key: DiagnosticKey) -> DiagnosticText {
    match key {
        DiagnosticKey::MakefileNotFound => DiagnosticText {
            message: "No se seleccionó ni se encontró un Makefile regular en la raíz del proyecto, así que las comprobaciones de objetivos de compilación y de lista de fuentes no se ejecutaron.",
            notes: &[
                "Esto es normal en un ejercicio de piscina, donde solo se esperan archivos .c: el Makefile y las cabeceras del proyecto son ambos opcionales.",
                "La ausencia nunca suspende. Solo la asignatura puede decir si se exige un Makefile, y normfix no lee asignaturas.",
            ],
            help: "Ignora esto cuando la asignatura espera archivos .c sueltos; añade o selecciona el Makefile cuando exija uno.",
        },
        DiagnosticKey::MakefileNotEvaluated => DiagnosticText {
            message: "Existe un Makefile regular en la raíz del proyecto pero no se seleccionó, así que preflight no lo evaluó.",
            notes: &[
                "Su cabecera, sus objetivos, sus recetas y sus referencias de fuentes están ausentes de este informe.",
            ],
            help: "Incluye el Makefile de la raíz explícitamente, o ejecuta preflight desde la raíz del proyecto sin un alcance parcial de archivos.",
        },
        DiagnosticKey::PreflightManualSteps => DiagnosticText {
            message: "Preflight no ejecuta recetas del proyecto, binarios, pruebas interactivas ni herramientas de fugas en tiempo de ejecución.",
            notes: &[
                "Ejecuta la secuencia de make/relink exigida por la asignatura y las pruebas funcionales en el entorno del evaluador.",
            ],
            help: "Completa las comprobaciones manuales específicas de la asignatura que muestra la ficha de evaluación antes de la defensa.",
        },
        DiagnosticKey::FunctionPolicyNotConfigured => DiagnosticText {
            message: "La comprobación de funciones autorizadas no está disponible porque este proyecto no tiene una lista permitida en normfix.toml.",
            notes: &[],
            help: "Crea normfix.toml a partir de la lista exacta de funciones autorizadas de la asignatura antes de confiar en preflight.",
        },
        DiagnosticKey::ReadmeCriteriaReview => DiagnosticText {
            message: "Hay un README, pero sus criterios de evaluación de 42 específicos de la asignatura no pueden probarse automáticamente.",
            notes: &[
                "La ausencia de README no suspende el preflight de normfix; este aviso solo existe cuando se encontró un README.",
            ],
            help: "Compáralo con la ficha de asignatura y de evaluación actual.",
        },
        DiagnosticKey::MakefileSourceNotFound => DiagnosticText {
            message: "La fuente literal `{source}` del Makefile no existe bajo la raíz del proyecto.",
            notes: &[
                "Solo se inspeccionó una asignación totalmente literal al estilo SRC/SRCS; las recetas y expansiones de Make nunca se ejecutaron.",
            ],
            help: "Crea o corrige la ruta de la fuente, o usa el modo de eliminación insegura explícitamente autorizado para quitar exactamente este token obsoleto.",
        },
        DiagnosticKey::MakefileSourceEmpty => DiagnosticText {
            message: "La fuente literal `{source}` del Makefile existe, pero no contiene ningún token C más allá de espacios y comentarios.",
            notes: &[
                "Una fuente vacía sigue enlazando, así que la compilación puede tener éxito mientras la funcionalidad que nombra no existe.",
            ],
            help: "Implementa el archivo, o usa el modo de eliminación insegura explícitamente autorizado para quitar exactamente este token.",
        },
        DiagnosticKey::HeaderPrototypeMissing => DiagnosticText {
            message: "El prototipo `{name}` de la cabecera no tiene implementación en el conjunto de fuentes del proyecto.",
            notes: &[
                "El conjunto completo y sin pérdidas de fuentes del proyecto no contiene ninguna definición no estática con este identificador. Las fuentes generadas y las bibliotecas externas no se infieren.",
            ],
            help: "Implementa la función declarada, o usa el modo inseguro explícitamente autorizado solo si esta API local del proyecto está intencionadamente sin uso.",
        },
        DiagnosticKey::HeaderPrototypeEmpty => DiagnosticText {
            message: "El prototipo `{name}` de la cabecera resuelve a una implementación sin ningún token C más allá de llaves, espacios y comentarios.",
            notes: &["Un no-op intencionado puede ser válido, así que normfix solo lo informa."],
            help: "Implementa el comportamiento exigido, o verifica con la asignatura que un cuerpo vacío es intencionado.",
        },
        DiagnosticKey::VlaCompatFalsePositive => DiagnosticText {
            message: "Norminette informó de un VLA, pero `{name}` resuelve a la constante entera {value}.",
            notes: &[
                "El límite es una constante de tiempo de compilación, así que esto es un falso positivo de compatibilidad, no un array de longitud variable.",
            ],
            help: "Nada que hacer; el array es C válido y válido según la Norma.",
        },
        DiagnosticKey::MarkdownCanonicalFormat => DiagnosticText {
            message: "El README se reimprimió como CommonMark canónico.",
            notes: &[],
            help: "Usa --check o --diff para previsualizar la reimpresión, o --no-format-markdown para mantener el documento byte a byte.",
        },
    }
}

// One arm per diagnostic: a long match is the price of making a missing
// translation impossible to merge.
#[allow(clippy::too_many_lines)]
fn french(key: DiagnosticKey) -> DiagnosticText {
    match key {
        DiagnosticKey::MakefileNotFound => DiagnosticText {
            message: "Aucun Makefile ordinaire n'a été sélectionné ni trouvé à la racine du projet, donc les vérifications de cibles de compilation et de liste de sources n'ont pas eu lieu.",
            notes: &[
                "C'est normal pour un exercice de piscine, où seuls des fichiers .c sont attendus : le Makefile comme les en-têtes du projet sont facultatifs.",
                "Une absence n'échoue jamais. Seul le sujet peut dire si un Makefile est exigé, et normfix ne lit pas les sujets.",
            ],
            help: "Ignorez ceci quand le sujet attend des fichiers .c isolés ; ajoutez ou sélectionnez le Makefile quand il en exige un.",
        },
        DiagnosticKey::MakefileNotEvaluated => DiagnosticText {
            message: "Un Makefile ordinaire existe à la racine du projet mais n'a pas été sélectionné, donc le preflight ne l'a pas évalué.",
            notes: &[
                "Son en-tête, ses cibles, ses recettes et ses références de sources sont absents de ce rapport.",
            ],
            help: "Incluez explicitement le Makefile de la racine, ou lancez le preflight depuis la racine du projet sans portée partielle de fichiers.",
        },
        DiagnosticKey::PreflightManualSteps => DiagnosticText {
            message: "Le preflight n'exécute ni recettes du projet, ni binaires, ni tests interactifs, ni outils de fuite à l'exécution.",
            notes: &[
                "Lancez la séquence make/relink exigée par le sujet et les tests fonctionnels dans l'environnement de l'évaluateur.",
            ],
            help: "Effectuez les vérifications manuelles propres au sujet indiquées dans la fiche d'évaluation avant la soutenance.",
        },
        DiagnosticKey::FunctionPolicyNotConfigured => DiagnosticText {
            message: "La vérification des fonctions autorisées est indisponible car ce projet n'a pas de liste autorisée dans normfix.toml.",
            notes: &[],
            help: "Créez normfix.toml à partir de la liste exacte des fonctions autorisées du sujet avant de vous fier au preflight.",
        },
        DiagnosticKey::ReadmeCriteriaReview => DiagnosticText {
            message: "Un README est présent, mais ses critères d'évaluation 42 propres au sujet ne peuvent pas être prouvés automatiquement.",
            notes: &[
                "L'absence de README ne fait pas échouer le preflight de normfix ; cet avis n'existe que lorsqu'un README a été trouvé.",
            ],
            help: "Comparez-le à la fiche de sujet et d'évaluation actuelle.",
        },
        DiagnosticKey::MakefileSourceNotFound => DiagnosticText {
            message: "La source littérale `{source}` du Makefile n'existe pas sous la racine du projet.",
            notes: &[
                "Seule une affectation entièrement littérale de style SRC/SRCS a été inspectée ; les recettes et expansions de Make n'ont jamais été exécutées.",
            ],
            help: "Créez ou corrigez le chemin de la source, ou utilisez le mode de suppression non sûre explicitement autorisé pour retirer exactement ce jeton obsolète.",
        },
        DiagnosticKey::MakefileSourceEmpty => DiagnosticText {
            message: "La source littérale `{source}` du Makefile existe, mais ne contient aucun jeton C au-delà des espaces et des commentaires.",
            notes: &[
                "Une source vide se lie quand même, donc la compilation peut réussir alors que la fonctionnalité qu'elle nomme n'existe pas.",
            ],
            help: "Implémentez le fichier, ou utilisez le mode de suppression non sûre explicitement autorisé pour retirer exactement ce jeton.",
        },
        DiagnosticKey::HeaderPrototypeMissing => DiagnosticText {
            message: "Le prototype `{name}` de l'en-tête n'a pas d'implémentation dans l'ensemble des sources du projet.",
            notes: &[
                "L'ensemble complet et sans perte des sources du projet ne contient aucune définition non statique portant cet identifiant. Les sources générées et les bibliothèques externes ne sont pas déduites.",
            ],
            help: "Implémentez la fonction déclarée, ou utilisez le mode non sûr explicitement autorisé uniquement si cette API locale au projet est volontairement inutilisée.",
        },
        DiagnosticKey::HeaderPrototypeEmpty => DiagnosticText {
            message: "Le prototype `{name}` de l'en-tête aboutit à une implémentation sans aucun jeton C au-delà des accolades, des espaces et des commentaires.",
            notes: &[
                "Un no-op intentionnel peut être valide, donc normfix se contente de le signaler.",
            ],
            help: "Implémentez le comportement attendu, ou vérifiez auprès du sujet qu'un corps vide est voulu.",
        },
        DiagnosticKey::VlaCompatFalsePositive => DiagnosticText {
            message: "Norminette a signalé un VLA, mais `{name}` vaut la constante entière {value}.",
            notes: &[
                "La borne est une constante de compilation : c'est donc un faux positif de compatibilité, pas un tableau de longueur variable.",
            ],
            help: "Rien à faire ; le tableau est du C valide et conforme à la Norme.",
        },
        DiagnosticKey::MarkdownCanonicalFormat => DiagnosticText {
            message: "Le README a été réimprimé en CommonMark canonique.",
            notes: &[],
            help: "Utilisez --check ou --diff pour prévisualiser la réimpression, ou --no-format-markdown pour garder le document octet pour octet.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{DIAGNOSTIC_KEYS, diagnostic_text};
    use crate::{Locale, PUBLISHED};

    fn placeholders(template: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = template;
        while let Some(start) = rest.find('{') {
            let Some(length) = rest[start..].find('}') else {
                break;
            };
            let end = start + length;
            names.push(rest[start + 1..end].to_owned());
            rest = &rest[end + 1..];
        }
        names.sort();
        names
    }

    #[test]
    fn no_locale_leaves_a_diagnostic_empty() {
        for locale in PUBLISHED {
            for key in DIAGNOSTIC_KEYS {
                let text = diagnostic_text(*locale, *key);
                assert!(
                    !text.message.trim().is_empty(),
                    "{}: {key:?} has no message",
                    locale.code()
                );
                assert!(
                    !text.help.trim().is_empty(),
                    "{}: {key:?} has no help",
                    locale.code()
                );
            }
        }
    }

    #[test]
    fn every_translation_keeps_the_english_placeholders_and_note_count() {
        for locale in PUBLISHED {
            for key in DIAGNOSTIC_KEYS {
                let english = diagnostic_text(Locale::English, *key);
                let translated = diagnostic_text(*locale, *key);
                assert_eq!(
                    placeholders(english.message),
                    placeholders(translated.message),
                    "{}: {key:?} changed its placeholders",
                    locale.code()
                );
                // A dropped note is a dropped caveat, which is the kind of
                // omission a reader cannot detect.
                assert_eq!(
                    english.notes.len(),
                    translated.notes.len(),
                    "{}: {key:?} changed its note count",
                    locale.code()
                );
            }
        }
    }

    #[test]
    fn a_translated_diagnostic_is_actually_translated() {
        for locale in PUBLISHED.iter().filter(|l| **l != Locale::English) {
            for key in DIAGNOSTIC_KEYS {
                assert_ne!(
                    diagnostic_text(Locale::English, *key).message,
                    diagnostic_text(*locale, *key).message,
                    "{}: {key:?} still carries the English message",
                    locale.code()
                );
            }
        }
    }
}
