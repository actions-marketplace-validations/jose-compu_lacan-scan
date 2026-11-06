# Intentional fail fixture for AIS108
def encode(tok, messages):
    text = tok.apply_chat_template(messages, tokenize=False)
    return tok(text, add_special_tokens=True)
