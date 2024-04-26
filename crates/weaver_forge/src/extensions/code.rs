// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to facilitate the generation of code.

/// Converts the input string into a comment with a prefix.
pub fn comment_with_prefix(input: &str, prefix: &str) -> String {
    let mut comment = String::new();
    for line in input.lines() {
        if !comment.is_empty() {
            comment.push_str("\n");
        }
        comment.push_str(&format!("{}{}", prefix, line));
    }
    comment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment() {
        assert_eq!(comment_with_prefix("test", "// "), "// test");

        let brief = r#"These attributes may be used to describe the client in a connection-based network interaction
where there is one side that initiates the connection (the client is the side that initiates the connection).
This covers all TCP network interactions since TCP is connection-based and one side initiates the
connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the
protocol / API doesn't expose a clear notion of client and server).
This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS."#;

      let expected_brief =  r#"/// These attributes may be used to describe the client in a connection-based network interaction
/// where there is one side that initiates the connection (the client is the side that initiates the connection).
/// This covers all TCP network interactions since TCP is connection-based and one side initiates the
/// connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the
/// protocol / API doesn't expose a clear notion of client and server).
/// This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS."#;

       assert_eq!(comment_with_prefix(brief, "/// "), expected_brief);
    }
}
