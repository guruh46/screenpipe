import { stat } from "@tauri-apps/plugin-fs";
import { platform } from "@tauri-apps/plugin-os";
import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export type FlattenObjectKeys<
  T extends Record<string, unknown>,
  Key = keyof T
> = Key extends string
  ? T[Key] extends Record<string, unknown> | undefined
    ? `${Key}.${FlattenObjectKeys<NonNullable<T[Key]>>}`
    : `${Key}`
  : never;

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
// Add this function to your existing utils.ts file
export function stripAnsiCodes(str: string): string {
  return str.replace(/\x1B\[[0-9;]*[JKmsu]/g, "");
}
export function toCamelCase(str: string): string {
  return str.replace(/([-_][a-z])/g, (group) =>
    group.toUpperCase().replace("-", "").replace("_", "")
  );
}

export function keysToCamelCase<T>(obj: any): T {
  if (Array.isArray(obj)) {
    return obj.map((v) => keysToCamelCase<T>(v)) as any;
  } else if (obj !== null && obj.constructor === Object) {
    return Object.keys(obj).reduce(
      (result, key) => ({
        ...result,
        [toCamelCase(key)]: keysToCamelCase(obj[key]),
      }),
      {}
    ) as T;
  }
  return obj;
}

export function encode(str: string): string {
  return encodeURIComponent(str).replace(/[!'()*]/g, (c) => {
    return "%" + c.charCodeAt(0).toString(16).toUpperCase();
  });
}
export const convertHtmlToMarkdown = (html: string) => {
  const convertedHtml = html.replace(
    /<img\s+(?:[^>]*?\s+)?src="([^"]*)"(?:\s+(?:[^>]*?\s+)?alt="([^"]*)")?\s*\/?>/g,
    (match, src, alt) => {
      return `![${alt || ""}](${src})`;
    }
  );
  return convertedHtml.replace(/<[^>]*>/g, "");
};

export function getCliPath() {
  const os = platform();
  switch (os) {
    case "windows":
      return "%LOCALAPPDATA%\\screenpipe\\screenpipe.exe";
    case "macos":
      return "/Applications/screenpipe.app/Contents/MacOS/screenpipe";
    case "linux":
      return "/usr/local/bin/screenpipe";
    default:
      return "screenpipe";
  }
}

export function parseKeyboardShortcut(shortcut: string): string {
  if (typeof window !== "undefined") {
    const os = platform();

    const uniqueKeys = new Set(
      shortcut
        .toLowerCase()
        .split("+")
        .map((key) => key.trim())
    );

    return Array.from(uniqueKeys)
      .map((key) => {
        if (key === "super") {
          return os === "macos" ? "⌘" : "⊞";
        }
        if (key === "ctrl") return "⌃";
        if (key === "alt") return os === "macos" ? "⌥" : "Alt";
        if (key === "shift") return "⇧";
        return key.charAt(0).toUpperCase() + key.slice(1);
      })
      .join(" + ");
  }
  return "";
}

export function stringToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  let color = "#";
  for (let i = 0; i < 3; i++) {
    const value = (hash >> (i * 8)) & 0xff;
    color += ("00" + value.toString(16)).substr(-2);
  }
  return color;
}

export async function getFileSize(filePath: string): Promise<number> {
  const { size } = await stat(filePath);

  return size;
}

// Helper functions to flatten/unflatten objects
export const flattenObject = (obj: any, prefix = ""): Record<string, any> => {
  return Object.keys(obj).reduce((acc: Record<string, any>, k: string) => {
    const pre = prefix.length ? prefix + "." : "";
    if (
      typeof obj[k] === "object" &&
      obj[k] !== null &&
      !Array.isArray(obj[k])
    ) {
      Object.assign(acc, flattenObject(obj[k], pre + k));
    } else {
      acc[pre + k] = obj[k];
    }
    return acc;
  }, {});
};

export const unflattenObject = (obj: Record<string, any>): any => {
  const result: any = {};
  for (const key in obj) {
    const keys = key.split(".");
    let current = result;
    for (let i = 0; i < keys.length; i++) {
      const k = keys[i];
      if (i === keys.length - 1) {
        current[k] = obj[key];
      } else {
        current[k] = current[k] || {};
        current = current[k];
      }
    }
  }
  return result;
};