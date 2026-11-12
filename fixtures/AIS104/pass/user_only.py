def build(retriever, query):
    docs = retriever.get_relevant_documents(query)
    user_message = f"Context: {docs}"
    system_prompt = "You are helpful"
    return system_prompt, user_message
