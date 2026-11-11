import * as child_process from "child_process";

export function run(client: any) {
  const resp = client.chat.completions.create({ messages: [] });
  const text = resp.choices[0].message.content;
  child_process.exec(text);
}
