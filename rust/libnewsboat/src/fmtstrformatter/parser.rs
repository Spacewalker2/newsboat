use nom::{is_digit, types::CompleteStr, IResult};
use std::str;

/// Describes all the different "format specifiers" we support, plus a chunk of text that would be
/// copied to the output verbatim.
#[derive(PartialEq, Eq, Debug)]
pub enum Specifier<'a> {
    /// Will expand to pad everything that comes next to the right. Given char is used for padding.
    Spacing(char),
    /// A format to be replaced with a value (`%a`, `%t` etc.), padded to the given width on the
    /// left (if it's positive) or on the right (if it's negative).
    Format(char, isize),
    /// A chunk of text that will be copied to the output verbatim.
    Text(&'a str),
    /// Conditional format that is replaced by one of the sub-formats depending on the value of the
    /// given key. "Else" branch might be missing.
    Conditional(char, Vec<Specifier<'a>>, Option<Vec<Specifier<'a>>>),
}

fn escaped_percent_sign(input: CompleteStr) -> IResult<CompleteStr, Specifier> {
    tag!(input, "%%").map(|result| {
        let CompleteStr(s) = result.1;
        (result.0, Specifier::Text(&s[0..1]))
    })
}

fn spacing(input: CompleteStr) -> IResult<CompleteStr, Specifier> {
    do_parse!(input, tag!("%>") >> c: take!(1) >> (c)).map(|result| {
        let CompleteStr(chr) = result.1;
        assert!(chr.len() == 1);
        let chr = chr.chars().next().unwrap();

        (result.0, Specifier::Spacing(chr))
    })
}

fn padded_format(input: CompleteStr) -> IResult<CompleteStr, Specifier> {
    do_parse!(
        input,
        tag!("%")
            >> width: take_while!(|chr| is_digit(chr as u8) || chr == '-')
            >> format: take!(1)
            >> (format, width)
    )
    .map(|result| {
        let (c, w) = result.1;

        let CompleteStr(format) = c;
        assert!(format.len() == 1);
        let format = format.chars().next().unwrap();

        let CompleteStr(width) = w;
        let width = width.parse::<isize>().unwrap_or(0);

        (result.0, Specifier::Format(format, width))
    })
}

fn text_outside_conditional(input: CompleteStr) -> IResult<CompleteStr, Specifier> {
    take_till!(input, |chr: char| chr == '%').map(|result| {
        let CompleteStr(s) = result.1;
        (result.0, Specifier::Text(s))
    })
}

fn text_inside_conditional(input: CompleteStr) -> IResult<CompleteStr, Specifier> {
    take_till!(input, |chr: char| chr == '%' || chr == '&' || chr == '?').map(|result| {
        let CompleteStr(s) = result.1;
        (result.0, Specifier::Text(s))
    })
}

fn conditional(input: CompleteStr) -> IResult<CompleteStr, Specifier> {
    do_parse!(
        input,
        tag!("%?")
            >> cond: take!(1)
            >> tag!("?")
            >> then: conditional_branch
            >> els: alt!(
            do_parse!(
                tag!("&") >>
                els: conditional_branch >>
                tag!("?") >>

                (Some(els))) |

            tag!("?") => { |_| None})
            >> (cond, then, els)
    )
    .map(|result| {
        let (cond, then, els) = result.1;

        let CompleteStr(cond) = cond;
        assert!(cond.len() == 1);
        let cond = cond.chars().next().unwrap();

        (result.0, Specifier::Conditional(cond, then, els))
    })
}

fn conditional_branch(input: CompleteStr) -> IResult<CompleteStr, Vec<Specifier>> {
    many0!(
        input,
        alt!(escaped_percent_sign | spacing | padded_format | text_inside_conditional)
    )
}

fn parser(input: CompleteStr) -> IResult<CompleteStr, Vec<Specifier>> {
    many0!(
        input,
        alt!(
            conditional | escaped_percent_sign | spacing | padded_format | text_outside_conditional
        )
    )
}

pub fn parse(input: &str) -> Vec<Specifier> {
    match parser(CompleteStr(input)) {
        Ok((_leftovers, ast)) => ast,
        Err(_) => vec![Specifier::Text("")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_parses_formats_without_specifiers() {
        let input = "Hello, world!";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();
        assert_eq!(leftovers, CompleteStr(""));
        assert_eq!(result, vec![Specifier::Text("Hello, world!")]);
    }

    #[test]
    fn t_replaces_double_percent_with_a_single_percent() {
        let input = "%%";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();
        assert_eq!(leftovers, CompleteStr(""));
        assert_eq!(result, vec![Specifier::Text("%")]);
    }

    #[test]
    fn t_parses_sequences_of_specifiers() {
        let input = "100%% pure Ceylon tea";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();
        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![
            Specifier::Text("100"),
            Specifier::Text("%"),
            Specifier::Text(" pure Ceylon tea"),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn t_parses_formats_with_letters() {
        let input = "%t (%a)";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();

        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![
            Specifier::Format('t', 0),
            Specifier::Text(" ("),
            Specifier::Format('a', 0),
            Specifier::Text(")"),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn t_parses_formats_with_positive_padding() {
        let input = "%8a%4b%13x";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();

        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![
            Specifier::Format('a', 8),
            Specifier::Format('b', 4),
            Specifier::Format('x', 13),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn t_parses_formats_with_negative_padding() {
        let input = "%-8a%-4b%-13x";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();

        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![
            Specifier::Format('a', -8),
            Specifier::Format('b', -4),
            Specifier::Format('x', -13),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn t_parses_spacing_format() {
        let input = "%-8a%>m%4b%> %-13x";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();

        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![
            Specifier::Format('a', -8),
            Specifier::Spacing('m'),
            Specifier::Format('b', 4),
            Specifier::Spacing(' '),
            Specifier::Format('x', -13),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn t_parses_conditionals() {
        let input = "%?x?success&failure?";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();

        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![Specifier::Conditional(
            'x',
            vec![Specifier::Text("success")],
            Some(vec![Specifier::Text("failure")]),
        )];
        assert_eq!(result, expected);
    }

    #[test]
    fn t_parses_conditionals_without_else_branch() {
        let input = "%?x?success?";
        let (leftovers, result) = parser(CompleteStr(input)).unwrap();

        assert_eq!(leftovers, CompleteStr(""));

        let expected = vec![Specifier::Conditional(
            'x',
            vec![Specifier::Text("success")],
            None,
        )];
        assert_eq!(result, expected);
    }
}
