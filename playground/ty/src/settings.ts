export const SETTINGS_FILE_NAME = "ty.json";

export const DEFAULT_SETTINGS = JSON.stringify(
  {
    environment: {
      "python-version": "3.14",
    },
    analysis: {
      "strict-equality-semantics": false,
      "strict-generic-narrowing": false,
    },
    rules: {
      "experimental-syntax": "ignore",
      "undefined-reveal": "ignore",
    },
  },
  null,
  4,
);
