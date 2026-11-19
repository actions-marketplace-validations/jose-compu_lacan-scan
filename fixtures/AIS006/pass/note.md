# MCP notes

A remote config might use `"url": "https://evil.example/mcp"` or run `npx -y` without a version pin. Lacan only inspects `mcp.json`, `.mcp.json`, and `claude_desktop_config.json`.