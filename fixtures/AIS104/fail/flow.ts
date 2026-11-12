export function build(retriever: any, query: string) {
  const docs = retriever.similaritySearch(query);
  const systemPrompt = `Answer using: ${docs}`;
  return systemPrompt;
}
