export type BrowserExtensionSource = "chromeStore" | "local";

export interface BrowserExtension {
  id: string;
  name: string;
  source: BrowserExtensionSource;
  storeUrl: string | null;
  storeId: string | null;
  version: string;
  installPath: string;
  defaultEnabled: boolean;
  profileIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ImportExtensionFromStoreInput {
  storeUrl: string;
  defaultEnabled: boolean;
  profileIds: string[];
}

export interface ImportExtensionFromLocalInput {
  localPath: string;
  defaultEnabled: boolean;
  profileIds: string[];
}

export interface UpdateExtensionInput {
  name: string;
  defaultEnabled: boolean;
  profileIds: string[];
}

export const extensionSourceLabelKey = (
  source: BrowserExtensionSource,
): "extensions.source.chromeStore" | "extensions.source.local" =>
  source === "chromeStore" ? "extensions.source.chromeStore" : "extensions.source.local";
