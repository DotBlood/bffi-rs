/**
 * The loader-schema validator used by the codegen CLI: a full
 * structural pass over the parsed JSON with precise `path: message`
 * diagnostics. Validation is value-level (unknown `abi`/`ts` names
 * are rejected) so the renderer can trust the shape afterwards.
 */
import {
  ABI_NAMES,
  OUT_NAMES,
  RET_ABI_NAMES,
  TS_NAMES,
} from "../loader/loader.ts";

/** One validation failure: the JSON path plus the reason. */
export interface SchemaIssue {
  path: string;
  message: string;
}

/** Raised by `validateModule`: `issues` is never empty. */
export class SchemaValidationError extends Error {
  readonly issues: SchemaIssue[];

  constructor(issues: SchemaIssue[]) {
    super(
      issues
        .map((issue) => `${issue.path}: ${issue.message}`)
        .join("\n"),
    );
    this.name = "SchemaValidationError";
    this.issues = issues;
  }
}

const STRING = "a string";
const STRING_ARRAY = "an array of strings";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function checkString(issues: SchemaIssue[], path: string, value: unknown): void {
  if (typeof value !== "string") {
    issues.push({ path, message: `expected ${STRING}` });
  }
}

function checkStringArray(issues: SchemaIssue[], path: string, value: unknown): void {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    issues.push({ path, message: `expected ${STRING_ARRAY}` });
  }
}

function checkEnum(
  issues: SchemaIssue[],
  path: string,
  value: unknown,
  allowed: ReadonlySet<string>,
): void {
  if (typeof value !== "string" || !allowed.has(value)) {
    issues.push({
      path,
      message: `unknown name ${JSON.stringify(value)} (expected one of ${[...allowed].join(", ")})`,
    });
  }
}

function checkParams(issues: SchemaIssue[], path: string, value: unknown): void {
  if (!Array.isArray(value)) {
    issues.push({ path, message: "expected an array of parameter entries" });
    return;
  }
  for (const [index, param] of value.entries()) {
    const at = `${path}[${index}]`;
    if (!isRecord(param)) {
      issues.push({ path: at, message: "expected an object" });
      continue;
    }
    checkString(issues, `${at}.name`, param.name);
    checkEnum(issues, `${at}.ts`, param.ts, TS_NAMES);
    checkEnum(issues, `${at}.abi`, param.abi, ABI_NAMES);
  }
}

function checkRet(issues: SchemaIssue[], path: string, value: unknown): void {
  if (!isRecord(value)) {
    issues.push({ path, message: "expected an object" });
    return;
  }
  checkEnum(issues, `${path}.ts`, value.ts, TS_NAMES);
  checkEnum(issues, `${path}.abi`, value.abi, RET_ABI_NAMES);
}

const OUT_NAMES_SET: ReadonlySet<string> = OUT_NAMES;

function checkFunctionLike(issues: SchemaIssue[], path: string, value: unknown): void {
  if (!isRecord(value)) {
    issues.push({ path, message: "expected an object" });
    return;
  }
  checkString(issues, `${path}.name`, value.name);
  checkString(issues, `${path}.export`, value.export);
  checkStringArray(issues, `${path}.docs`, value.docs);
  checkParams(issues, `${path}.params`, value.params);
  checkRet(issues, `${path}.ret`, value.ret);
  if (value.out !== undefined) {
    checkEnum(issues, `${path}.out`, value.out, OUT_NAMES_SET);
  }
}

function checkClass(issues: SchemaIssue[], path: string, value: unknown): void {
  if (!isRecord(value)) {
    issues.push({ path, message: "expected an object" });
    return;
  }
  checkString(issues, `${path}.name`, value.name);
  checkString(issues, `${path}.release`, value.release);
  checkStringArray(issues, `${path}.docs`, value.docs);
  checkFunctionLike(issues, `${path}.constructor`, value.constructor);
  if (!Array.isArray(value.fields)) {
    issues.push({ path: `${path}.fields`, message: "expected an array of field entries" });
  } else {
    for (const [index, field] of value.fields.entries()) {
      const at = `${path}.fields[${index}]`;
      if (!isRecord(field)) {
        issues.push({ path: at, message: "expected an object" });
        continue;
      }
      checkString(issues, `${at}.name`, field.name);
      checkString(issues, `${at}.export`, field.export);
      checkStringArray(issues, `${at}.docs`, field.docs);
      checkEnum(issues, `${at}.ts`, field.ts, TS_NAMES);
      checkEnum(issues, `${at}.out`, field.out, OUT_NAMES_SET);
    }
  }
  if (!Array.isArray(value.methods)) {
    issues.push({ path: `${path}.methods`, message: "expected an array of method entries" });
  } else {
    for (const [index, method] of value.methods.entries()) {
      checkFunctionLike(issues, `${path}.methods[${index}]`, method);
    }
  }
}

/**
 * Validates the raw parsed JSON against loader schema v1 and returns
 * it typed. Throws [`SchemaValidationError`] with every issue found.
 */
export function validateModule(raw: unknown): ModuleJsonLike {
  const issues: SchemaIssue[] = [];
  if (!isRecord(raw)) {
    throw new SchemaValidationError([
      { path: "$", message: "expected a JSON object" },
    ]);
  }
  if (raw.bffi !== 1) {
    issues.push({
      path: "$.bffi",
      message: `unsupported schema version ${JSON.stringify(raw.bffi)} (expected 1)`,
    });
  }
  checkString(issues, "$.module", raw.module);
  if (!Array.isArray(raw.functions)) {
    issues.push({ path: "$.functions", message: "expected an array" });
  } else {
    for (const [index, fn] of raw.functions.entries()) {
      checkFunctionLike(issues, `$.functions[${index}]`, fn);
    }
  }
  if (!Array.isArray(raw.classes)) {
    issues.push({ path: "$.classes", message: "expected an array" });
  } else {
    for (const [index, cls] of raw.classes.entries()) {
      checkClass(issues, `$.classes[${index}]`, cls);
    }
  }
  if (issues.length > 0) {
    throw new SchemaValidationError(issues);
  }
  return raw as unknown as ModuleJsonLike;
}

/** The minimal structural type the validator guarantees. */
export interface ModuleJsonLike {
  bffi: number;
  module: string;
  functions: unknown[];
  classes: unknown[];
}
