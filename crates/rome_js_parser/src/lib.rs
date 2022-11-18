//! Extremely fast, lossless, and error tolerant JavaScript Parser.
//!
//! The parser uses an abstraction over non-whitespace tokens.
//! This allows us to losslessly or lossly parse code without requiring explicit handling of whitespace.
//! The parser yields events, not an AST, the events are resolved into untyped syntax nodes, which can then
//! be casted into a typed AST.
//!
//! The parser is able to produce a valid AST from **any** source code.
//! Erroneous productions are wrapped into `ERROR` syntax nodes, the original source code
//! is completely represented in the final syntax nodes.
//!
//! You probably do not want to use the parser struct, unless you want to parse fragments of Js source code or make your own productions.
//! Instead use functions such as [parse_script], [parse_module], and [] which offer abstracted versions for parsing.
//!
//! Notable features of the parser are:
//! - Extremely fast parsing and lexing through the extremely fast lexer.
//! - Ability to do Lossy or Lossless parsing on demand without explicit whitespace handling.
//! - Customizable, able to parse any fragments of JS code at your discretion.
//! - Completely error tolerant, able to produce an AST from any source code.
//! - Zero cost for converting untyped nodes to a typed AST.
//! - Ability to go from AST to SyntaxNodes to SyntaxTokens to source code and back very easily with nearly zero cost.
//! - Very easy tree traversal through [`SyntaxNode`](rome_rowan::SyntaxNode).
//! - Descriptive errors with multiple labels and notes.
//! - Very cheap cloning, cloning an ast node or syntax node is the cost of adding a reference to an Rc.
//! - Cheap incremental reparsing of changed text.
//!
//! The crate further includes utilities such as:
//! - ANSI syntax highlighting of nodes or text through `lexer`.
//!
//! It is inspired by the rust analyzer parser but adapted for JavaScript.
//!
//! # Syntax Nodes vs AST Nodes
//! The crate relies on a concept of untyped [rome_js_syntax::JsSyntaxNode]s vs typed [rome_rowan::AstNode]s.
//! Syntax nodes represent the syntax tree in an untyped way. They represent a location in an immutable
//! tree with two pointers. The syntax tree is composed of [rome_js_syntax::JsSyntaxNode]s and [rome_js_syntax::JsSyntaxToken]s in a nested
//! tree structure. Each node can have parents, siblings, children, descendants, etc.
//!
//! [rome_rowan::AstNode]s represent a typed version of a syntax node. They have the same exact representation as syntax nodes
//! therefore a conversion between either has zero runtime cost. Every piece of data of an ast node is optional,
//! this is due to the fact that the parser is completely error tolerant.
//!
//! Each representation has its advantages:
//!
//! ### SyntaxNodes
//! - Very simple traversing of the syntax tree through functions on them.
//! - Easily able to convert to underlying text, range, or tokens.
//! - Contain all whitespace bound to the underlying production (in the case of lossless parsing).
//! - Can be easily converted into its typed representation with zero cost.
//! - Can be turned into a pretty representation with fmt debug.
//!
//! ### AST Nodes
//! - Easy access to properties of the underlying production.
//! - Zero cost conversion to a syntax node.
//!
//! In conclusion, the use of both representations means we are not constrained to acting through
//! typed nodes. Which makes traversal hard and you often have to resort to autogenerated visitor patterns.
//! AST nodes are simply a way to easily access subproperties of a syntax node.event;
//!
//!
//! # Authoring Parse Rules
//!
//! This is a short, or not so short, guide to implement parse rules using the Rome parser infrastructure.
//!
//! ## Naming
//! The convention is to prefix your parse rule with `parse_` and then use the name defined in the grammar file.
//!
//! For example, `parse_for_statement` or `parse_expression`.
//!
//! ## Signature
//! Most parse rules take a `&mut` reference to the parser as their only parameter and return a `ParsedSyntax`.
//!
//! ```rust,ignore
//! fn parse_rule_name(&mut: Parser) -> ParsedSyntax {}
//! ```
//!
//! You're free to add additional parameters to your function if needed. There are rare cases where you want to consider returning `ConditionalParsedSyntax` as explained in [conditional syntax](#conditional-syntax)
//!
//!
//! ## Parsing a single node
//!
//! Let's assume you want to parse the JS `if` statement:
//!
//! ```js
//! JsIfStatement =
//!  if
//!  (
//!  test: JsAnyExpression
//!  )
//!  consequent: JsBlockStatement
//!  else_clause: JsElseClause?
//! ```
//!
//! ### Presence Test
//!
//! Now, the parsing function must first test if the parser is positioned at an `if` statement and return `Absent` if that's not the case.
//!
//! ```rust, ignore
//! if !p.at(T![if]) {
//!  return ParsedSyntax::Absent;
//! }
//! ```
//!
//! Why return `ParsedSyntax::Absent`? The function must return `ParsedSyntax::Absent` if the rule can't predict by the next token(s) if they form the expected node or not. Doing so allows the calling rule to decide if this is an error and perform an error recovery if necessary.  The second reason is to ensure that the rule doesn't return a node where all children are missing.
//!
//! Your rule implementation may want to consider more than just the first child to determine if it can parse at least some of the expected children.
//! For example, the if statement rule could test if the parser is located at an `else` clause and then create an `if` statement where all children are missing except the `else` clause:
//!
//! ```rust, ignore
//! if !p.at(T![if]) && !p.at(T![else]){
//!   return Absent
//! }
//! ```
//!
//! Your implementation can also call into another parsing rule if the first child is a node and not a token.
//!
//! ```rust, ignore
//! let assignment_target = parse_assignment_target(p);
//!
//! if assignment_target.is_absent() {
//!   return Absent;
//! }
//!
//! let my_node = assignment_target.precede_or_missing();
//! ```
//!
//! But be careful with calling other rules. Your rule mustn't progress the parser - meaning that it can't
//! advance in the parsing process and consume tokens - if it returns `Absent`.
//!
//!
//! ### Parse children
//! The parse rules will guide you in how to write your implementation and the parser infrastructure provides the following convenience APIs:
//!
//! * Optional token `'ident'?`: Use `p.eat(token)`. It eats the next token if it matches the passed-in token.
//! * Required token `'ident'`: Use`p.expect(token)`. It eats the next token if it matches the passed-in token.
//! It adds an `Expected 'x' but found 'y' instead` error and a missing marker if the token isn't present in the source code.
//! * Optional node `body: JsBlockStatement?`: Use`parse_block_statement(p).or_missing(p)`. It parses the block if it is present in the source code and adds a missing marker if it isn't.
//! * Required node `body: JsBlockStatement`: Use `parse_block_statement(p).or_missing_with_error(p, error_builder)`:
//! it parses the block statement if it is present in the source code and adds a missing marker and an error if not.
//!
//! Using the above-described rules result in the following implementation for the `if` statement rule.
//!
//! ```rust, ignore
//! fn parse_if_statement(p: &mut Parser) -> ParsedSyntax {
//!  if !p.at(T![if]) {
//!   return Absent;
//!  }
//!
//!  let m = p.start();
//!
//!  p.expect(T![if]);
//!  p.expect(T!['(']);
//!  parse_any_expression(p).or_add_diagnostic(p, js_parse_errors::expeced_if_statement);
//!  p.expect(T![')']);
//!  parse_block_statement(p).or_add_diagnostic(p, js_parse_errors::expected_block_statement);
//! // the else block is optional, handle the marker by using `ok`
//!  parse_else_clause(p).ok();
//!
//!  Present(m.complete(p, JS_IF_STATEMENT));
//! }
//! ```
//!
//! Hold on, what are these *missing* markers? Rome's AST facade uses fixed offsets to retrieve a particular child from a node.
//! For example, the 3rd child of the if statement is the condition. However, the condition would become the second element
//! if the opening parentheses `(` isn't present in the source text. That's where missing elements come into play.
//!
//! ## Parsing Lists & Error Recovery
//!
//! Parsing lists is different from parsing single elements with a fixed set of children because it requires looping until
//! the parser reaches a terminal token (or the end of the file).
//!
//! You may remember that `parse_*` methods shouldn't progress parsing if they return `Absent`.
//! Not progressing the parser is problematic inside `while` loops because it inevitably results in an infinite loop.
//!
//! That's why you must do error recovery when parsing lists. Luckily, the parser comes with the infrastructure to make error recovery a piece of cake.
//! The general structure for parsing a list is (yes, that's something the parser infrastructure should provide for you):
//!
//!
//! Let's try to parse an array:
//!
//! ```js
//! [ 1, 3, 6 ]
//! ```
//!
//! We will use  `ParseSeparatedList` in order to achieve that
//!
//! ```rust, ignore
//! struct ArrayElementsList;
//!
//! impl ParseSeparatedList for ArrayElementsList {
//!     type ParsedElement = CompletedMarker;
//!
//!     fn parse_element(&mut self, p: &mut Parser) -> ParsedSyntax<Self::ParsedElement> {
//!         parse_array_element(p)
//!     }
//!
//!     fn is_at_list_end(&self, p: &mut Parser) -> bool {
//!         p.at_ts(token_set![T![default], T![case], T!['}']])
//!     }
//!
//!     fn recover(
//!         &mut self,
//!         p: &mut Parser,
//!         parsed_element: ParsedSyntax<Self::ParsedElement>,
//!     ) -> parser::RecoveryResult {
//!         parsed_element.or_recover(
//!             p,
//!             &ParseRecovery::new(JS_UNKNOWN_STATEMENT, STMT_RECOVERY_SET),
//!             js_parse_error::expected_case,
//!         )
//!     }
//! };
//! ```
//!
//! Let's run through this step by step:
//!
//! ```rust, ignore
//! parsed_element.or_recover(
//!     p,
//!     &ParseRecovery::new(JS_UNKNOWN_STATEMENT, STMT_RECOVERY_SET),
//!     js_parse_error::expected_case,
//! )
//! ```
//!
//! The `or_recover` performs an error recovery if the `parse_array_element` method returns `Absent`;
//! there's no array element in the source text.
//!
//! The recovery eats all tokens until it finds one of the tokens specified in the `token_set`,
//! a line break (if you called `enable_recovery_on_line_break`) or the end of the file.
//!
//! The recovery doesn't throw the tokens away but instead wraps them inside a `UNKNOWN_JS_EXPRESSION` node (first parameter).
//! There exist multiple `UNKNOWN_*` nodes. You must consult the grammar to understand which `UNKNOWN*` node is supported in your case.
//!
//! > You usually want to include the terminal token ending your list, the element separator token, and the token terminating a statement in your recovery set.
//!
//!
//! Now, the problem with recovery is that it can fail, and there are two reasons:
//!
//! - the parser reached the end of the file;
//! - the next token is one of the tokens specified in the recovery set, meaning there is nothing to recover from;
//!
//! In these cases the `ParseSeparatedList` and `ParseNodeList` will recover the parser for you.
//!
//! ## Conditional Syntax
//!
//! The conditional syntax allows you to express that some syntax may not be valid in all source files. Some use cases are:
//!
//! * syntax that is only supported in strict or sloppy mode: for example, `with` statements is not valid when a JavaScript file uses `"use strict"` or is a module;
//! * syntax that is only supported in certain file types: Typescript, JSX, modules;
//! * syntax that is only available in specific language versions: experimental features, different versions of the language e.g. (ECMA versions for JavaScript);
//!
//! The idea is that the parser always parses the syntax regardless of whatever it is supported in this specific file or context.
//! The main motivation behind doing so is that this gives us perfect error recovery and allows us to use the same code regardless of whether the syntax is supported.
//!
//! However, conditional syntax must be handled because we want to add a diagnostic if the syntax isn't supported for the current file, and the parsed tokens must be attached somewhere.
//!
//! Let's have a look at the `with` statement that is only allowed in loose mode/sloppy mode:
//!
//! ```rust, ignore
//! fn parse_with_statement(p: &mut Parser) -> ParsedSyntax {
//!  if !p.at(T![with]) {
//!   return Absent;
//!  }
//!
//!  let m = p.start();
//!  p.bump(T![with]); // with
//!  parenthesized_expression(p).or_add_diagnostic(p, js_errors::expected_parenthesized_expression);
//!  parse_statement(p).or_add_diagnostic(p, js_error::expected_statement);
//!  let with_stmt = m.complete(p, JS_WITH_STATEMENT);
//!
//!  let conditional = StrictMode.excluding_syntax(p, with_stmt, |p, marker| {
//!   p.err_builder("`with` statements are not allowed in strict mode", marker.range(p))
//!  });
//!
//!
//! }
//! ```
//!
//! The start of the rule is the same as for any other rule. The exciting bits start with
//!
//! ```rust, ignore
//! let conditional = StrictMode.excluding_syntax(p, with_stmt, |p, marker| {
//!  p.err_builder("`with` statements are not allowed in strict mode", marker.range(p))
//! });
//! ```
//!
//! The `StrictMode.excluding_syntax` converts the parsed syntax to an unknown node and uses the diagnostic builder to create a diagnostic if the feature is not supported.
//!
//! You can convert the `ConditionalParsedSyntax` to a regular `ParsedSyntax` by calling `or_invalid_to_unknown`, which wraps the whole parsed `with` statement in an `UNKNOWN` node if the parser is in strict mode and otherwise returns the unchanged `with` statement.
//!
//! What if there's no `UNKNOWN` node matching the node of your parse rule? You must then return the `ConditionalParsedSyntax` without making the `or_invalid_to_unknown` recovery. It's then up to the caller to recover the potentially invalid syntax.
//!
//!
//! ## Summary
//!
//! * Parse rules are named `parse_rule_name`
//! * The parse rules should return a `ParsedSyntax`
//! * The rule must return `Present` if it consumes any token and, therefore, can parse the node with at least some of its children.
//! * It returns `Absent` otherwise and must not progress parsing nor add any errors.
//! * Lists must perform error recovery to avoid infinite loops.
//! * Consult the grammar to identify the `UNKNOWN` node that is valid in the context of your rule.
//!
//! ## Parser Tests
//!
//! Parser tests are comments that start with `test` or `test_err` followed by the test name, and then the code on its own line.
//!
//! ```rust,ignore
//! // test feature_name
//! // let a = { new_feature : "" }
//! // let b = { new_feature : "" }
//! fn parse_new_feature(p: &mut Parser) -> ParsedSyntax {}
//! ```
//!
//! * `test`: Test for a valid program. Should not produce any diagnostics nor missing nodes.
//! * `test_err`: Test for a program with syntax error. Must produce a diagnostic.
//!
//! By default, the test runs as a JavaScript Module. You can customize the source type by specifying the
//! file type after `test` or `test_err`
//!
//! ```rust,ignore
//! // test ts typescript_test
//! // console.log("a");
//! if a {
//!     // ..
//! }
//! ```
//!
//! The supported source types are:
//! * `js`
//! * `jsx`
//! * `ts`
//! * `tsx`
//! * `d.ts`
//!
//! To enable script mode, add a `// script` comment to the code.
//!
//! To extract the test cases, run `cargo codegen test`. Running the codegen is necessary whenever you add,
//! change, or remove inline tests .
//!
//! To update the test output, run
//!
//!
//! **Linux/MacOs**:
//!
//! ```bash
//! env UPDATE_EXPECT=1 cargo test
//! ```
//!
//! **Windows**
//!
//! ```powershell
//! set UPDATE_EXPECT=1 & cargo test
//! ```

mod parser;
#[macro_use]
mod token_set;
mod event;
mod lexer;
mod lossless_tree_sink;
mod parse;
mod span;
mod state;

#[cfg(any(test, feature = "tests"))]
pub mod test_utils;
#[cfg(test)]
mod tests;

pub mod syntax;
mod token_source;

use crate::parser::ToDiagnostic;
pub(crate) use crate::parser::{ParseNodeList, ParseSeparatedList, ParsedSyntax};
pub(crate) use crate::ParsedSyntax::{Absent, Present};
pub use crate::{
    event::{process, Event},
    lexer::{LexContext, ReLexContext},
    lossless_tree_sink::LosslessTreeSink,
    parse::*,
    token_set::TokenSet,
};
pub(crate) use parser::{Checkpoint, CompletedMarker, Marker, ParseRecovery, Parser};
use rome_console::fmt::Display;
use rome_console::MarkupBuf;
use rome_diagnostics::console::markup;
use rome_diagnostics::location::AsSpan;
use rome_diagnostics::{
    Advices, Diagnostic, FileId, Location, LogCategory, MessageAndDescription, Visit,
};
use rome_js_syntax::{JsSyntaxKind, LanguageVariant};
use rome_rowan::{TextRange, TextSize};
pub(crate) use state::{ParserState, StrictMode};
use std::fmt::Debug;

/// A specialized diagnostic for the parser
///
/// Parser diagnostics are always **errors**.
///
/// A parser diagnostics structured in this way:
/// 1. a mandatory message and a mandatory [TextRange]
/// 2. a list of details, useful to give more information and context around the error
/// 3. a hint, which should tell the user how they could fix their issue
///
/// These information **are printed in this exact order**.
///
#[derive(Debug, Diagnostic, Clone)]
#[diagnostic(category = "parse", severity = Error)]
pub struct ParseDiagnostic {
    /// The location where the error is occurred
    #[location(span)]
    span: Option<TextRange>,
    /// Reference to a file where the issue occurred
    #[location(resource)]
    file_id: FileId,
    #[message]
    #[description]
    message: MessageAndDescription,
    #[advice]
    advice: ParserAdvice,
}

/// Possible details related to the diagnostic
#[derive(Debug, Default, Clone)]
struct ParserAdvice {
    /// A list a possible details that can be attached to the diagnostic.
    /// Useful to explain the nature errors.
    detail_list: Vec<ParserAdviceDetail>,
    /// A message for the user that should tell the user how to fix the issue
    hint: Option<MarkupBuf>,
}

/// The structure of the advice. A message that gives details, a possible range so
/// the diagnostic is able to highlight the part of the code we want to explain.
#[derive(Debug, Clone)]
struct ParserAdviceDetail {
    /// A message that should explain this detail
    message: MarkupBuf,
    /// An optional range that should highlight the details of the code
    span: Option<TextRange>,
    /// The file id, reference to the actual file
    file_id: FileId,
}

impl ParserAdvice {
    fn add_detail(&mut self, message: impl Display, range: Option<TextRange>, file_id: FileId) {
        self.detail_list.push(ParserAdviceDetail {
            message: markup! { {message} }.to_owned(),
            span: range,
            file_id,
        });
    }

    fn add_hint(&mut self, message: impl Display) {
        self.hint = Some(markup! { { message } }.to_owned());
    }
}

impl Advices for ParserAdvice {
    fn record(&self, visitor: &mut dyn Visit) -> std::io::Result<()> {
        for detail in &self.detail_list {
            let ParserAdviceDetail {
                span,
                message,
                file_id,
            } = detail;
            visitor.record_log(LogCategory::Info, &markup! { {message} }.to_owned())?;
            let location = Location::builder().span(span).resource(file_id).build();
            if let Some(location) = location {
                visitor.record_frame(location)?;
            }
        }
        if let Some(hint) = &self.hint {
            visitor.record_log(LogCategory::Info, &markup! { {hint} }.to_owned())?;
        }
        Ok(())
    }
}

impl ParseDiagnostic {
    pub fn new(file_id: FileId, message: impl Display, span: impl AsSpan) -> Self {
        Self {
            file_id,
            span: span.as_span(),
            message: MessageAndDescription::from(markup! { {message} }.to_owned()),
            advice: ParserAdvice::default(),
        }
    }

    pub const fn is_error(&self) -> bool {
        true
    }

    /// Use this API if you want to highlight more code frame, to help to explain where's the error.
    ///
    /// A detail is printed **after the actual error** and before the hint.
    ///
    /// ## Examples
    ///
    /// ```
    /// use rome_console::fmt::{Termcolor};
    /// use rome_console::markup;
    /// use rome_diagnostics::{DiagnosticExt, FileId, PrintDiagnostic, console::fmt::Formatter};
    /// use rome_js_parser::ParseDiagnostic;
    /// use rome_js_syntax::TextRange;
    /// use rome_rowan::TextSize;
    /// use std::fmt::Write;
    ///
    /// let source = "const a";
    /// let range = TextRange::new(TextSize::from(0), TextSize::from(5));
    /// let mut diagnostic = ParseDiagnostic::new(FileId::zero(), "this is wrong!", range)
    ///     .detail(TextRange::new(TextSize::from(6), TextSize::from(7)), "This is reason why it's broken");
    ///
    /// let mut write = rome_diagnostics::termcolor::Buffer::no_color();
    /// let error = diagnostic
    ///     .clone()
    ///     .with_file_path(FileId::zero())
    ///     .with_file_source_code(source.to_string());
    /// Formatter::new(&mut Termcolor(&mut write))
    ///     .write_markup(markup! {
    ///     {PrintDiagnostic(&error, true)}
    /// })
    ///     .expect("failed to emit diagnostic");
    ///
    /// let mut result = String::new();
    /// write!(
    ///     result,
    ///     "{}",
    ///     std::str::from_utf8(write.as_slice()).expect("non utf8 in error buffer")
    /// ).expect("");
    ///
    /// let expected = r#"parse ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    ///
    ///   × this is wrong!
    ///  
    ///   > 1 │ const a
    ///       │ ^^^^^
    ///  
    ///   i This is reason why it's broken
    ///  
    ///   > 1 │ const a
    ///       │       ^
    ///  
    /// "#;
    /// assert_eq!(result, expected);
    pub fn detail(mut self, range: impl AsSpan, message: impl Display) -> Self {
        self.advice
            .add_detail(message, range.as_span(), self.file_id);
        self
    }

    /// Small message that should suggest the user how they could fix the error
    ///
    /// Hints are rendered a **last part** of the diagnostics
    ///
    /// ## Examples
    ///
    /// ```
    /// use rome_console::fmt::{Termcolor};
    /// use rome_console::markup;
    /// use rome_diagnostics::{DiagnosticExt, FileId, PrintDiagnostic, console::fmt::Formatter};
    /// use rome_js_parser::ParseDiagnostic;
    /// use rome_js_syntax::TextRange;
    /// use rome_rowan::TextSize;
    /// use std::fmt::Write;
    ///
    /// let source = "const a";
    /// let range = TextRange::new(TextSize::from(0), TextSize::from(5));
    /// let mut diagnostic = ParseDiagnostic::new(FileId::zero(), "this is wrong!", range)
    ///     .hint("You should delete the code");
    ///
    /// let mut write = rome_diagnostics::termcolor::Buffer::no_color();
    /// let error = diagnostic
    ///     .clone()
    ///     .with_file_path(FileId::zero())
    ///     .with_file_source_code(source.to_string());
    /// Formatter::new(&mut Termcolor(&mut write))
    ///     .write_markup(markup! {
    ///     {PrintDiagnostic(&error, true)}
    /// })
    ///     .expect("failed to emit diagnostic");
    ///
    /// let mut result = String::new();
    /// write!(
    ///     result,
    ///     "{}",
    ///     std::str::from_utf8(write.as_slice()).expect("non utf8 in error buffer")
    /// ).expect("");
    ///
    /// let expected = r#"parse ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    ///
    ///   × this is wrong!
    ///  
    ///   > 1 │ const a
    ///       │ ^^^^^
    ///  
    ///   i You should delete the code
    ///  
    /// "#;
    /// assert_eq!(result, expected);
    /// ```
    ///
    pub fn hint(mut self, message: impl Display) -> Self {
        self.advice.add_hint(message);
        self
    }

    /// Retrieves the range that belongs to the diagnostic
    fn diagnostic_range(&self) -> Option<&TextRange> {
        self.span.as_ref()
    }
}

/// An abstraction for syntax tree implementations
pub trait TreeSink {
    /// Adds new token to the current branch.
    fn token(&mut self, kind: JsSyntaxKind, end: TextSize);

    /// Start new branch and make it current.
    fn start_node(&mut self, kind: JsSyntaxKind);

    /// Finish current branch and restore previous
    /// branch as current.
    fn finish_node(&mut self);

    /// Emit errors
    fn errors(&mut self, errors: Vec<ParseDiagnostic>);
}

/// A syntax feature that may or may not be supported depending on the file type and parser configuration
pub(crate) trait SyntaxFeature: Sized {
    /// Returns `true` if the current parsing context supports this syntax feature.
    fn is_supported(&self, p: &Parser) -> bool;

    /// Returns `true` if the current parsing context doesn't support this syntax feature.
    fn is_unsupported(&self, p: &Parser) -> bool {
        !self.is_supported(p)
    }

    /// Adds a diagnostic and changes the kind of the node to [SyntaxKind::to_unknown] if this feature isn't
    /// supported.
    ///
    /// Returns the parsed syntax.
    fn exclusive_syntax<S, E, D>(&self, p: &mut Parser, syntax: S, error_builder: E) -> ParsedSyntax
    where
        S: Into<ParsedSyntax>,
        E: FnOnce(&Parser, &CompletedMarker) -> D,
        D: ToDiagnostic,
    {
        syntax.into().map(|mut syntax| {
            if self.is_unsupported(p) {
                let error = error_builder(p, &syntax);
                p.error(error);
                syntax.change_to_unknown(p);
                syntax
            } else {
                syntax
            }
        })
    }

    /// Parses a syntax and adds a diagnostic and changes the kind of the node to [SyntaxKind::to_unknown] if this feature isn't
    /// supported.
    ///
    /// Returns the parsed syntax.
    fn parse_exclusive_syntax<P, E>(
        &self,
        p: &mut Parser,
        parse: P,
        error_builder: E,
    ) -> ParsedSyntax
    where
        P: FnOnce(&mut Parser) -> ParsedSyntax,
        E: FnOnce(&Parser, &CompletedMarker) -> ParseDiagnostic,
    {
        if self.is_supported(p) {
            parse(p)
        } else {
            let diagnostics_checkpoint = p.diagnostics.len();
            let syntax = parse(p);
            p.diagnostics.truncate(diagnostics_checkpoint);

            match syntax {
                Present(mut syntax) => {
                    let diagnostic = error_builder(p, &syntax);
                    p.error(diagnostic);
                    syntax.change_to_unknown(p);
                    Present(syntax)
                }
                _ => Absent,
            }
        }
    }

    /// Adds a diagnostic and changes the kind of the node to [SyntaxKind::to_unknown] if this feature is
    /// supported.
    ///
    /// Returns the parsed syntax.
    fn excluding_syntax<S, E>(&self, p: &mut Parser, syntax: S, error_builder: E) -> ParsedSyntax
    where
        S: Into<ParsedSyntax>,
        E: FnOnce(&Parser, &CompletedMarker) -> ParseDiagnostic,
    {
        syntax.into().map(|mut syntax| {
            if self.is_unsupported(p) {
                syntax
            } else {
                let error = error_builder(p, &syntax);
                p.error(error);
                syntax.change_to_unknown(p);
                syntax
            }
        })
    }
}

pub enum JsSyntaxFeature {
    #[allow(unused)]
    #[doc(alias = "LooseMode")]
    SloppyMode,
    StrictMode,
    TypeScript,
    Jsx,
}

impl SyntaxFeature for JsSyntaxFeature {
    fn is_supported(&self, p: &Parser) -> bool {
        match self {
            JsSyntaxFeature::SloppyMode => p.state.strict().is_none(),
            JsSyntaxFeature::StrictMode => p.state.strict().is_some(),
            JsSyntaxFeature::TypeScript => p.source_type.language().is_typescript(),
            JsSyntaxFeature::Jsx => p.source_type.variant() == LanguageVariant::Jsx,
        }
    }
}
