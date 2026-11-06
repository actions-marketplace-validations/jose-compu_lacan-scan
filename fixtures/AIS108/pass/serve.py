# Flattened chat text is tokenized without re-parsing special tokens
def encode(tok, messages):
    text = tok.apply_chat_template(messages, tokenize=False)
    return tok(text, add_special_tokens=False)
