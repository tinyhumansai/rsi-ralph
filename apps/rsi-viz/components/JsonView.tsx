/** Pretty-printed JSON, truncated so a huge snapshot cannot stall the page. */
const MAX_CHARS = 200_000;

export function JsonView({ value }: { value: unknown }) {
  const text = JSON.stringify(value, null, 2) ?? "null";
  const truncated = text.length > MAX_CHARS;
  return (
    <>
      <pre className="json">{truncated ? text.slice(0, MAX_CHARS) : text}</pre>
      {truncated ? (
        <p className="muted" style={{ fontSize: "0.8rem" }}>
          Showing the first {MAX_CHARS.toLocaleString()} characters; open the file link for the rest.
        </p>
      ) : null}
    </>
  );
}
