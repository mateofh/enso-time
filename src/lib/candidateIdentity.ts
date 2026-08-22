const FNV_OFFSET = BigInt("0xcbf29ce484222325");
const FNV_PRIME = BigInt("0x100000001b3");
const encoder = new TextEncoder();

export function canonicalCandidateLabel(label: string): string {
  return label.trim().replace(/[A-Z]/g, (character) => character.toLowerCase());
}

export function candidateId(label: string): string {
  let hash = FNV_OFFSET;
  for (const byte of encoder.encode(canonicalCandidateLabel(label))) {
    hash = BigInt.asUintN(64, (hash ^ BigInt(byte)) * FNV_PRIME);
  }
  return `candidate-${hash.toString(16).padStart(16, "0")}`;
}

export function compareUtf8(left: string, right: string): number {
  const leftBytes = encoder.encode(left);
  const rightBytes = encoder.encode(right);
  const sharedLength = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < sharedLength; index += 1) {
    const difference = leftBytes[index]! - rightBytes[index]!;
    if (difference) return difference;
  }
  return leftBytes.length - rightBytes.length;
}
