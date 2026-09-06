/**
 * Type-level consumer of the generated `api.d.ts` (criteria 6.1/6.2):
 * this file exists for `bun run typecheck` (tsc) alone. `import type`
 * is erased at runtime (spike-verified: a type-only import of a
 * non-existent specifier never reaches module resolution), so no test
 * imports this file and tsc alone guards the declared surface.
 */
import type { add, checked_div, is_even, mirror_i64, mirror_u64 } from "./api";

/** Compile-time assertion helper: `T` must be exactly `true`. */
export type Expect<T extends boolean> = T;

/** Compile-time equality: `X` and `Y` must be the same type. */
export type Equal<X, Y> =
  (<T>() => T extends X ? 1 : 2) extends <T>() => T extends Y ? 1 : 2
    ? true
    : false;

// add: (a: number, b: number) => number
export const addReturnsNumber: Expect<Equal<ReturnType<typeof add>, number>> = true;
export const addTakesNumbers: Expect<
  Equal<Parameters<typeof add>, [a: number, b: number]>
> = true;

// mirror_i64: (x: bigint) => bigint
export const mirrorI64TakesBigint: Expect<
  Equal<Parameters<typeof mirror_i64>[0], bigint>
> = true;
export const mirrorI64ReturnsBigint: Expect<
  Equal<ReturnType<typeof mirror_i64>, bigint>
> = true;

// mirror_u64: (x: bigint) => bigint
export const mirrorU64TakesBigint: Expect<
  Equal<Parameters<typeof mirror_u64>[0], bigint>
> = true;
export const mirrorU64ReturnsBigint: Expect<
  Equal<ReturnType<typeof mirror_u64>, bigint>
> = true;

// is_even: (x: bigint) => boolean
export const isEvenTakesBigint: Expect<
  Equal<Parameters<typeof is_even>[0], bigint>
> = true;
export const isEvenReturnsBoolean: Expect<
  Equal<ReturnType<typeof is_even>, boolean>
> = true;

// checked_div: (a: number, b: number) => number (the Err channel
// travels through the last-error mechanism, not the TS signature)
export const checkedDivReturnsNumber: Expect<
  Equal<ReturnType<typeof checked_div>, number>
> = true;

// Structural direction checks: the imported functions fit where their
// declared signatures belong and NOT where they do not.
export const addFitsBinaryNumber: Expect<
  Equal<typeof add extends (a: number, b: number) => number ? true : false, true>
> = true;
export const addDoesNotTakeBigint: Expect<
  Equal<typeof add extends (a: bigint, b: number) => number ? true : false, false>
> = true;
export const mirrorI64DoesNotTakeNumber: Expect<
  Equal<typeof mirror_i64 extends (x: number) => bigint ? true : false, false>
> = true;

/** A helper whose parameter must accept the imported `add`. */
export function applyAdd(fn: (a: number, b: number) => number): number {
  return fn(2, 3);
}
export const applyAddAcceptsAdd: Expect<
  Equal<Parameters<typeof applyAdd>[0], typeof add>
> = true;
