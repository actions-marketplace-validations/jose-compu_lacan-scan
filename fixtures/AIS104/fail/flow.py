def build(retriever, query):
    docs = retriever.get_relevant_documents(query)
    system_prompt = f"Answer using: {docs}"
    return system_prompt
