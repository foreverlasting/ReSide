export const cn = (...xs: Array<string | false | null | undefined>): string =>
  xs.filter(Boolean).join(" ");
