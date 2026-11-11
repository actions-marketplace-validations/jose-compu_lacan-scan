def run(client):
    resp = client.chat.completions.create(model="x", messages=[])
    text = resp.choices[0].message.content
    eval(text)
