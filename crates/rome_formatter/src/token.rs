use crate::prelude::*;
use crate::{
    write, Argument, Arguments, CommentKind, CstFormatContext, DanglingTrivia, FormatRefWithRule,
    GroupId, SourceComment,
};
use rome_rowan::{Language, SyntaxNode, SyntaxToken};

///! Provides builders for working with tokens and the tokens trivia

/// Formats the leading comments of `node`
pub const fn format_leading_comments<L: Language>(
    node: &SyntaxNode<L>,
) -> FormatLeadingComments<L> {
    FormatLeadingComments::Node(node)
}

/// Formats the leading comments of a node.
#[derive(Debug, Copy, Clone)]
pub enum FormatLeadingComments<'a, L: Language> {
    Node(&'a SyntaxNode<L>),
    Comments(&'a [SourceComment<L>]),
}

impl<Context> Format<Context> for FormatLeadingComments<'_, Context::Language>
where
    Context: CstFormatContext,
{
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        let comments = f.context().comments().clone();

        let leading_comments = match self {
            FormatLeadingComments::Node(node) => comments.leading_comments(node),
            FormatLeadingComments::Comments(comments) => comments,
        };

        for comment in leading_comments {
            let format_comment = FormatRefWithRule::new(comment, Context::CommentRule::default());
            write!(f, [format_comment])?;

            match comment.kind() {
                CommentKind::Block | CommentKind::InlineBlock => {
                    match comment.lines_after() {
                        0 => write!(f, [space()])?,
                        1 => {
                            if comment.lines_before() == 0 {
                                write!(f, [soft_line_break_or_space()])?;
                            } else {
                                write!(f, [hard_line_break()])?;
                            }
                        }
                        _ => write!(f, [empty_line()])?,
                    };
                }
                CommentKind::Line => match comment.lines_after() {
                    0 | 1 => write!(f, [hard_line_break()])?,
                    _ => write!(f, [empty_line()])?,
                },
            }
        }

        Ok(())
    }
}

/// Formats the trailing comments of `node`.
pub const fn format_trailing_comments<L: Language>(
    node: &SyntaxNode<L>,
) -> FormatTrailingComments<L> {
    FormatTrailingComments::Node(node)
}

/// Formats the trailing comments of `node`
#[derive(Debug, Clone, Copy)]
pub enum FormatTrailingComments<'a, L: Language> {
    Node(&'a SyntaxNode<L>),
    Comments(&'a [SourceComment<L>]),
}

impl<Context> Format<Context> for FormatTrailingComments<'_, Context::Language>
where
    Context: CstFormatContext,
{
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        let comments = f.context().comments().clone();
        let trailing_comments = match self {
            FormatTrailingComments::Node(node) => comments.trailing_comments(node),
            FormatTrailingComments::Comments(comments) => comments,
        };

        let mut total_lines_before = 0;

        for comment in trailing_comments {
            total_lines_before += comment.lines_before();

            let format_comment = FormatRefWithRule::new(comment, Context::CommentRule::default());

            // This allows comments at the end of nested structures:
            // {
            //   x: 1,
            //   y: 2
            //   // A comment
            // }
            // Those kinds of comments are almost always leading comments, but
            // here it doesn't go "outside" the block and turns it into a
            // trailing comment for `2`. We can simulate the above by checking
            // if this a comment on its own line; normal trailing comments are
            // always at the end of another expression.
            if total_lines_before > 0 {
                write!(
                    f,
                    [
                        line_suffix(&format_with(|f| {
                            match comment.lines_before() {
                                0 | 1 => write!(f, [hard_line_break()])?,
                                _ => write!(f, [empty_line()])?,
                            };

                            write!(f, [format_comment])
                        })),
                        expand_parent()
                    ]
                )?;
            } else {
                let content = format_with(|f| write!(f, [space(), format_comment]));
                if comment.kind().is_line() {
                    write!(f, [line_suffix(&content), expand_parent()])?;
                } else {
                    write!(f, [content])?;
                }
            }
        }

        Ok(())
    }
}

pub const fn format_dangling_trivia<L: Language>(
    token: &SyntaxToken<L>,
) -> FormatDanglingTrivia<L> {
    FormatDanglingTrivia {
        token,
        indent: false,
        ignore_formatted_check: false,
    }
}

/// Formats the dangling trivia of `token`.
pub struct FormatDanglingTrivia<'a, L: Language> {
    token: &'a SyntaxToken<L>,
    indent: bool,
    ignore_formatted_check: bool,
}

impl<L: Language> FormatDanglingTrivia<'_, L> {
    pub fn indented(mut self) -> Self {
        self.indent = true;
        self
    }

    pub fn ignore_formatted_check(mut self) -> Self {
        self.ignore_formatted_check = true;
        self
    }
}

impl<Context> Format<Context> for FormatDanglingTrivia<'_, Context::Language>
where
    Context: CstFormatContext,
{
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        if !self.ignore_formatted_check && f.state().is_token_trivia_formatted(self.token) {
            return Ok(());
        }

        let comments = f.context().comments().clone();
        let dangling_trivia = comments.dangling_trivia(self.token);
        let mut leading_comments_end = 0;
        let mut last_line_comment = false;

        let format_leading_comments = format_once(|f| {
            if self.indent && matches!(dangling_trivia.first(), Some(DanglingTrivia::Comment(_))) {
                write!(f, [hard_line_break()])?;
            }

            // Write all comments up to the first skipped token trivia or the token
            let mut join = f.join_with(hard_line_break());

            for trivia in dangling_trivia {
                match trivia {
                    DanglingTrivia::Comment(comment) => {
                        let format_comment =
                            FormatRefWithRule::new(comment, Context::CommentRule::default());
                        join.entry(&format_comment);

                        last_line_comment = comment.kind().is_line();
                        leading_comments_end += 1;
                    }
                    _ => {
                        break;
                    }
                }
            }

            join.finish()
        });

        if self.indent {
            write!(f, [block_indent(&format_leading_comments)])?;
        } else {
            write!(f, [format_leading_comments])?;

            if last_line_comment {
                write!(f, [hard_line_break()])?;
            }
        }

        if leading_comments_end != dangling_trivia.len() {
            panic!("Skipped token trivia not yet supported");
        }

        f.state_mut().mark_token_trivia_formatted(self.token);

        Ok(())
    }
}

/// Formats a token without its leading or trailing trivia
///
/// ## Warning
/// It's your responsibility to format leading or trailing comments and skipped trivia.
pub const fn format_trimmed_token<L: Language>(token: &SyntaxToken<L>) -> FormatTrimmedToken<L> {
    FormatTrimmedToken { token }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct FormatTrimmedToken<'a, L: Language> {
    token: &'a SyntaxToken<L>,
}

impl<L: Language + 'static, C> Format<C> for FormatTrimmedToken<'_, L>
where
    C: CstFormatContext<Language = L>,
{
    fn fmt(&self, f: &mut Formatter<C>) -> FormatResult<()> {
        let trimmed_range = self.token.text_trimmed_range();
        syntax_token_text_slice(self.token, trimmed_range).fmt(f)
    }
}
/// Formats the leading and trailing trivia of a removed token.
///
/// Formats all leading and trailing comments up to the first line break or skipped token trivia as a trailing
/// comment of the previous token. The remaining trivia is then printed as leading trivia of the next token.
pub const fn format_removed<L>(token: &SyntaxToken<L>) -> FormatRemoved<L>
where
    L: Language,
{
    FormatRemoved { token }
}

/// Formats the trivia of a token that is present in the source text but should be omitted in the
/// formatted output.
pub struct FormatRemoved<'a, L>
where
    L: Language,
{
    token: &'a SyntaxToken<L>,
}

impl<C, L> Format<C> for FormatRemoved<'_, L>
where
    L: Language + 'static,
    C: CstFormatContext<Language = L>,
{
    fn fmt(&self, f: &mut Formatter<C>) -> FormatResult<()> {
        f.state_mut().track_token(self.token);

        write!(f, [format_dangling_trivia(self.token)])
    }
}

/// Print out a `token` from the original source with a different `content`.
///
/// This will print the trivia that belong to `token` to `content`;
/// `token` is then marked as consumed by the formatter.
pub fn format_replaced<'a, 'content, L, Context>(
    token: &'a SyntaxToken<L>,
    content: &'content impl Format<Context>,
) -> FormatReplaced<'a, 'content, L, Context>
where
    L: Language,
{
    FormatReplaced {
        token,
        content: Argument::new(content),
    }
}

/// Formats a token's leading and trailing trivia but uses the provided content instead
/// of the token in the formatted output.
#[derive(Copy, Clone)]
pub struct FormatReplaced<'a, 'content, L, C>
where
    L: Language,
{
    token: &'a SyntaxToken<L>,
    content: Argument<'content, C>,
}

impl<L, C> Format<C> for FormatReplaced<'_, '_, L, C>
where
    L: Language + 'static,
    C: CstFormatContext<Language = L>,
{
    fn fmt(&self, f: &mut Formatter<C>) -> FormatResult<()> {
        f.state_mut().track_token(self.token);

        write!(f, [format_dangling_trivia(self.token)])?;

        f.write_fmt(Arguments::from(&self.content))
    }
}

/// Formats the given token only if the group does break and otherwise retains the token's trivia.
pub fn format_only_if_breaks<'a, 'content, L, Content, Context>(
    token: &'a SyntaxToken<L>,
    content: &'content Content,
) -> FormatOnlyIfBreaks<'a, 'content, L, Context>
where
    L: Language,
    Content: Format<Context>,
{
    FormatOnlyIfBreaks {
        token,
        content: Argument::new(content),
        group_id: None,
    }
}

/// Formats a token with its leading and trailing trivia that only gets printed if its enclosing
/// group does break but otherwise gets omitted from the formatted output.
pub struct FormatOnlyIfBreaks<'a, 'content, L, C>
where
    L: Language,
{
    token: &'a SyntaxToken<L>,
    content: Argument<'content, C>,
    group_id: Option<GroupId>,
}

impl<'a, 'content, L, C> FormatOnlyIfBreaks<'a, 'content, L, C>
where
    L: Language,
{
    pub fn with_group_id(mut self, group_id: Option<GroupId>) -> Self {
        self.group_id = group_id;
        self
    }
}

impl<L, C> Format<C> for FormatOnlyIfBreaks<'_, '_, L, C>
where
    L: Language + 'static,
    C: CstFormatContext<Language = L>,
{
    fn fmt(&self, f: &mut Formatter<C>) -> FormatResult<()> {
        write!(
            f,
            [
                if_group_breaks(&Arguments::from(&self.content)).with_group_id(self.group_id),
                // Print the trivia otherwise
                if_group_fits_on_line(&format_dangling_trivia(self.token))
                    .with_group_id(self.group_id),
            ]
        )
    }
}
