export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    // Body wrapping is stylistic and must not block releases of published commits.
    'body-max-line-length': [0],
  },
};
