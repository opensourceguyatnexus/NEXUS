// The NEXUS logo: a heavy N with one cut corner, the mark that shows pin 1 on a chip.
// It takes the current text color, so it works on every theme.

export function LogoMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 256 256" fill="currentColor" aria-hidden="true">
      <path d="M24 68 68 24H88L172 150V24H232V232H172L84 106V232H24Z" />
    </svg>
  );
}
