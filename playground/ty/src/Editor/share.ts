/**
 * Self-contained share links: the workspace is deflated and base64url-encoded
 * into the URL fragment, so sharing needs no backend and the fragment never
 * leaves the browser.
 */

export type Playground = {
  files: { [name: string]: string };
  /// the name of the current file
  current: string;
};

const PREFIX = "v1.";

/**
 * Encodes a playground into a URL fragment (without the leading `#`).
 */
export async function encodePlayground(
  playground: Playground,
): Promise<string> {
  const json = new TextEncoder().encode(JSON.stringify(playground));
  const deflated = await transform(json, new CompressionStream("deflate-raw"));
  return PREFIX + toBase64Url(deflated);
}

/**
 * Decodes a URL fragment (without the leading `#`) produced by
 * {@link encodePlayground}. Returns `null` if the fragment is not a share link.
 */
export async function decodePlayground(
  fragment: string,
): Promise<Playground | null> {
  if (!fragment.startsWith(PREFIX)) {
    return null;
  }

  const deflated = fromBase64Url(fragment.slice(PREFIX.length));
  const json = await transform(
    deflated,
    new DecompressionStream("deflate-raw"),
  );
  const value: unknown = JSON.parse(new TextDecoder().decode(json));
  if (!isPlayground(value)) {
    throw new Error("Share link does not contain a valid workspace");
  }
  return value;
}

async function transform(
  bytes: Uint8Array<ArrayBuffer>,
  stream: CompressionStream | DecompressionStream,
): Promise<Uint8Array<ArrayBuffer>> {
  const output = new Blob([bytes]).stream().pipeThrough(stream);
  return new Uint8Array(await new Response(output).arrayBuffer());
}

function toBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/, "");
}

function fromBase64Url(text: string): Uint8Array<ArrayBuffer> {
  const binary = atob(text.replaceAll("-", "+").replaceAll("_", "/"));
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

function isPlayground(value: unknown): value is Playground {
  if (typeof value !== "object" || value == null) {
    return false;
  }
  const { files, current } = value as Record<string, unknown>;
  return (
    typeof current === "string" &&
    typeof files === "object" &&
    files != null &&
    Object.values(files).every((content) => typeof content === "string") &&
    Object.hasOwn(files, current)
  );
}
