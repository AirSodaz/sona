//! JSON repair for partial or malformed LLM responses.
//!
//! Port of `fix-json.ts` from the Vercel AI SDK and aimux-core.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FixState {
    Root,
    Finish,
    InsideString,
    InsideStringEscape,
    InsideStringUnicodeEscape,
    InsideLiteral,
    InsideNumber,
    InsideObjectStart,
    InsideObjectKey,
    InsideObjectAfterKey,
    InsideObjectBeforeValue,
    InsideObjectAfterValue,
    InsideObjectAfterComma,
    InsideArrayStart,
    InsideArrayAfterValue,
    InsideArrayAfterComma,
}

struct Fixer<'a> {
    chars: &'a [char],
    stack: Vec<FixState>,
    last_valid_index: isize,
    literal_start: Option<usize>,
    unicode_escape_digits: usize,
}

impl<'a> Fixer<'a> {
    fn new(chars: &'a [char]) -> Self {
        Self {
            chars,
            stack: vec![FixState::Root],
            last_valid_index: -1,
            literal_start: None,
            unicode_escape_digits: 0,
        }
    }

    fn is_hex_digit(c: char) -> bool {
        c.is_ascii_hexdigit()
    }

    fn process_value_start(&mut self, c: char, i: usize, swap_state: FixState) {
        match c {
            '"' => {
                self.last_valid_index = i as isize;
                self.stack.pop();
                self.stack.push(swap_state);
                self.stack.push(FixState::InsideString);
            }
            'f' | 't' | 'n' => {
                self.last_valid_index = i as isize;
                self.literal_start = Some(i);
                self.stack.pop();
                self.stack.push(swap_state);
                self.stack.push(FixState::InsideLiteral);
            }
            '-' => {
                self.stack.pop();
                self.stack.push(swap_state);
                self.stack.push(FixState::InsideNumber);
            }
            '0'..='9' => {
                self.last_valid_index = i as isize;
                self.stack.pop();
                self.stack.push(swap_state);
                self.stack.push(FixState::InsideNumber);
            }
            '{' => {
                self.last_valid_index = i as isize;
                self.stack.pop();
                self.stack.push(swap_state);
                self.stack.push(FixState::InsideObjectStart);
            }
            '[' => {
                self.last_valid_index = i as isize;
                self.stack.pop();
                self.stack.push(swap_state);
                self.stack.push(FixState::InsideArrayStart);
            }
            _ => {}
        }
    }

    fn process_after_object_value(&mut self, c: char, i: usize) {
        match c {
            ',' => {
                self.stack.pop();
                self.stack.push(FixState::InsideObjectAfterComma);
            }
            '}' => {
                self.last_valid_index = i as isize;
                self.stack.pop();
            }
            _ => {}
        }
    }

    fn process_after_array_value(&mut self, c: char, i: usize) {
        match c {
            ',' => {
                self.stack.pop();
                self.stack.push(FixState::InsideArrayAfterComma);
            }
            ']' => {
                self.last_valid_index = i as isize;
                self.stack.pop();
            }
            _ => {}
        }
    }

    fn run(&mut self) {
        for (i, &c) in self.chars.iter().enumerate() {
            let current = *self
                .stack
                .last()
                .expect("fix-json stack is never empty (seeded with Root)");

            match current {
                FixState::Root => self.process_value_start(c, i, FixState::Finish),

                FixState::InsideObjectStart => match c {
                    '"' => {
                        self.stack.pop();
                        self.stack.push(FixState::InsideObjectKey);
                    }
                    '}' => {
                        self.last_valid_index = i as isize;
                        self.stack.pop();
                    }
                    _ => {}
                },

                FixState::InsideObjectAfterComma => {
                    if c == '"' {
                        self.stack.pop();
                        self.stack.push(FixState::InsideObjectKey);
                    }
                }

                FixState::InsideObjectKey => {
                    if c == '"' {
                        self.stack.pop();
                        self.stack.push(FixState::InsideObjectAfterKey);
                    }
                }

                FixState::InsideObjectAfterKey => {
                    if c == ':' {
                        self.stack.pop();
                        self.stack.push(FixState::InsideObjectBeforeValue);
                    }
                }

                FixState::InsideObjectBeforeValue => {
                    self.process_value_start(c, i, FixState::InsideObjectAfterValue);
                }

                FixState::InsideObjectAfterValue => {
                    self.process_after_object_value(c, i);
                }

                FixState::InsideString => match c {
                    '"' => {
                        self.stack.pop();
                        self.last_valid_index = i as isize;
                    }
                    '\\' => {
                        self.stack.push(FixState::InsideStringEscape);
                    }
                    _ => {
                        self.last_valid_index = i as isize;
                    }
                },

                FixState::InsideArrayStart => match c {
                    ']' => {
                        self.last_valid_index = i as isize;
                        self.stack.pop();
                    }
                    _ => {
                        self.last_valid_index = i as isize;
                        self.process_value_start(c, i, FixState::InsideArrayAfterValue);
                    }
                },

                FixState::InsideArrayAfterValue => match c {
                    ',' => {
                        self.stack.pop();
                        self.stack.push(FixState::InsideArrayAfterComma);
                    }
                    ']' => {
                        self.last_valid_index = i as isize;
                        self.stack.pop();
                    }
                    _ => {
                        self.last_valid_index = i as isize;
                    }
                },

                FixState::InsideArrayAfterComma => {
                    self.process_value_start(c, i, FixState::InsideArrayAfterValue);
                }

                FixState::InsideStringEscape => {
                    self.stack.pop();
                    if c == 'u' {
                        self.unicode_escape_digits = 0;
                        self.stack.push(FixState::InsideStringUnicodeEscape);
                    } else {
                        self.last_valid_index = i as isize;
                    }
                }

                FixState::InsideStringUnicodeEscape => {
                    if Self::is_hex_digit(c) {
                        self.unicode_escape_digits += 1;
                        if self.unicode_escape_digits == 4 {
                            self.stack.pop();
                            self.last_valid_index = i as isize;
                        }
                    }
                }

                FixState::InsideNumber => match c {
                    '0'..='9' => {
                        self.last_valid_index = i as isize;
                    }
                    'e' | 'E' | '-' | '.' => {}
                    ',' => {
                        self.stack.pop();
                        if self.stack.last() == Some(&FixState::InsideArrayAfterValue) {
                            self.process_after_array_value(c, i);
                        }
                        if self.stack.last() == Some(&FixState::InsideObjectAfterValue) {
                            self.process_after_object_value(c, i);
                        }
                    }
                    '}' => {
                        self.stack.pop();
                        if self.stack.last() == Some(&FixState::InsideObjectAfterValue) {
                            self.process_after_object_value(c, i);
                        }
                    }
                    ']' => {
                        self.stack.pop();
                        if self.stack.last() == Some(&FixState::InsideArrayAfterValue) {
                            self.process_after_array_value(c, i);
                        }
                    }
                    _ => {
                        self.stack.pop();
                    }
                },

                FixState::InsideLiteral => {
                    let start = self
                        .literal_start
                        .expect("literal_start is set whenever INSIDE_LITERAL is pushed");
                    let partial: String = self.chars[start..=i].iter().collect();
                    if !"false".starts_with(&partial)
                        && !"true".starts_with(&partial)
                        && !"null".starts_with(&partial)
                    {
                        self.stack.pop();
                        match self.stack.last() {
                            Some(FixState::InsideObjectAfterValue) => {
                                self.process_after_object_value(c, i);
                            }
                            Some(FixState::InsideArrayAfterValue) => {
                                self.process_after_array_value(c, i);
                            }
                            _ => {}
                        }
                    } else {
                        self.last_valid_index = i as isize;
                    }
                }

                FixState::Finish => {}
            }
        }
    }

    fn finish(self) -> String {
        let mut result: String = if self.last_valid_index >= 0 {
            self.chars[..=(self.last_valid_index as usize)]
                .iter()
                .collect()
        } else {
            String::new()
        };

        for state in self.stack.iter().rev() {
            match state {
                FixState::InsideString => result.push('"'),
                FixState::InsideObjectKey
                | FixState::InsideObjectAfterKey
                | FixState::InsideObjectAfterComma
                | FixState::InsideObjectStart
                | FixState::InsideObjectBeforeValue
                | FixState::InsideObjectAfterValue => result.push('}'),
                FixState::InsideArrayStart
                | FixState::InsideArrayAfterComma
                | FixState::InsideArrayAfterValue => result.push(']'),
                FixState::InsideLiteral => {
                    let start = self
                        .literal_start
                        .expect("literal_start is set whenever INSIDE_LITERAL is pushed");
                    let partial: String = self.chars[start..].iter().collect();
                    let plen = partial.chars().count();
                    if "true".starts_with(&partial) {
                        result.push_str(&"true"[plen..]);
                    } else if "false".starts_with(&partial) {
                        result.push_str(&"false"[plen..]);
                    } else if "null".starts_with(&partial) {
                        result.push_str(&"null"[plen..]);
                    }
                }
                _ => {}
            }
        }

        result
    }
}

/// Repairs a (possibly partial or truncated) JSON string into valid JSON.
#[must_use]
pub fn fix_json(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut fixer = Fixer::new(&chars);
    fixer.run();
    fixer.finish()
}
