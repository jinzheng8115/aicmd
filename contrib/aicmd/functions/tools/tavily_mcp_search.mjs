import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const query = process.argv.slice(2).join(" ").trim();
if (!query) {
  console.error("usage: tavily_mcp_search.mjs <query>");
  process.exit(1);
}

const transport = new StdioClientTransport({
  command: "npx",
  args: ["-y", "tavily-mcp"],
  stderr: "pipe",
  env: process.env,
});

const client = new Client({ name: "aicmd-tavily-client", version: "0.1.0" }, { capabilities: {} });
await client.connect(transport);

try {
  const result = await client.callTool({
    name: "tavily_search",
    arguments: {
      query,
      topic: "general",
      search_depth: "advanced",
      max_results: 5,
      include_answer: true,
      include_raw_content: false,
      include_images: false,
      include_image_descriptions: false,
    },
  });

  const text = (result.content || [])
    .map((item) => (typeof item.text === "string" ? item.text : ""))
    .filter(Boolean)
    .join("\n");

  process.stdout.write(text || "");
} finally {
  await client.close();
}
