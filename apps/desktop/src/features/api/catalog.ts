import type { MessageKey } from "@veil/i18n";

export type ApiMethod = "GET" | "POST" | "PATCH" | "PUT" | "DELETE";
export type ApiProtection = "standard" | "sensitive";

export interface ApiParam {
  name: string;
  type: string;
  required: boolean;
  example: string;
}

export interface ApiEndpoint {
  id: string;
  groupId: ApiGroupId;
  titleKey: MessageKey;
  descriptionKey: MessageKey;
  method: ApiMethod;
  path: string;
  protection: ApiProtection;
  queryParams?: ApiParam[];
  bodyExample?: string;
  responseExample: string;
}

export type ApiGroupId =
  | "temporary"
  | "profiles"
  | "cookies"
  | "fingerprints"
  | "groups"
  | "extensions"
  | "cache"
  | "credentials";

export interface ApiGroup {
  id: ApiGroupId;
  titleKey: MessageKey;
}

export const apiGroups: ApiGroup[] = [
  { id: "profiles", titleKey: "api.group.profiles" },
  { id: "temporary", titleKey: "api.group.temporary" },
  { id: "cookies", titleKey: "api.group.cookies" },
  { id: "fingerprints", titleKey: "api.group.fingerprints" },
  { id: "groups", titleKey: "api.group.groups" },
  { id: "extensions", titleKey: "api.group.extensions" },
  { id: "cache", titleKey: "api.group.cache" },
  { id: "credentials", titleKey: "api.group.credentials" },
];

export const apiEndpoints: ApiEndpoint[] = [
  {
    id: "profiles.list",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.list",
    descriptionKey: "api.endpoint.profiles.list.description",
    method: "GET",
    path: "/v1/profiles",
    protection: "standard",
    queryParams: [
      { name: "groupId", type: "string", required: false, example: "group_123" },
      { name: "status", type: "string", required: false, example: "running" },
      { name: "keyword", type: "string", required: false, example: "storefront" },
    ],
    responseExample: `{
  "success": true,
  "data": [
    {
      "id": "profile_123",
      "name": "Storefront US",
      "status": "idle",
      "groupId": "group_123"
    }
  ]
}`,
  },
  {
    id: "profiles.get",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.get",
    descriptionKey: "api.endpoint.profiles.get.description",
    method: "GET",
    path: "/v1/profiles/{profileId}",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": {
    "id": "profile_123",
    "name": "Storefront US",
    "fingerprint": {},
    "proxyId": null
  }
}`,
  },
  {
    id: "profiles.running",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.running",
    descriptionKey: "api.endpoint.profiles.running.description",
    method: "GET",
    path: "/v1/running-profiles",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": [{ "id": "profile_123", "processId": 4321 }]
}`,
  },
  {
    id: "profiles.isRunning",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.isRunning",
    descriptionKey: "api.endpoint.profiles.isRunning.description",
    method: "GET",
    path: "/v1/profiles/{profileId}/running",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": { "profileId": "profile_123", "running": true }
}`,
  },
  {
    id: "profiles.create",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.create",
    descriptionKey: "api.endpoint.profiles.create.description",
    method: "POST",
    path: "/v1/profiles",
    protection: "standard",
    bodyExample: `{
  "name": "Storefront US",
  "os": "windows",
  "platform": "Storefront",
  "groupId": null,
  "proxyId": null
}`,
    responseExample: `{
  "success": true,
  "data": { "id": "profile_123", "name": "Storefront US" }
}`,
  },
  {
    id: "profiles.update",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.update",
    descriptionKey: "api.endpoint.profiles.update.description",
    method: "PATCH",
    path: "/v1/profiles/{profileId}",
    protection: "standard",
    bodyExample: `{
  "name": "Storefront US Updated",
  "notes": "Updated through local API"
}`,
    responseExample: `{
  "success": true,
  "data": { "id": "profile_123", "name": "Storefront US Updated" }
}`,
  },
  {
    id: "profiles.delete",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.delete",
    descriptionKey: "api.endpoint.profiles.delete.description",
    method: "DELETE",
    path: "/v1/profiles/{profileId}",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": { "deleted": true }
}`,
  },
  {
    id: "profiles.launch",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.launch",
    descriptionKey: "api.endpoint.profiles.launch.description",
    method: "POST",
    path: "/v1/profiles/{profileId}/launch",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": { "kind": "started", "profileId": "profile_123", "processId": 4321 }
}`,
  },
  {
    id: "profiles.close",
    groupId: "profiles",
    titleKey: "api.endpoint.profiles.close",
    descriptionKey: "api.endpoint.profiles.close.description",
    method: "POST",
    path: "/v1/profiles/{profileId}/close",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": { "id": "profile_123", "status": "idle" }
}`,
  },
  {
    id: "temporary.launch",
    groupId: "temporary",
    titleKey: "api.endpoint.temporary.launch",
    descriptionKey: "api.endpoint.temporary.launch.description",
    method: "POST",
    path: "/v1/temp-profiles/launch",
    protection: "standard",
    bodyExample: `{
  "profileId": "profile_123"
}`,
    responseExample: `{
  "success": true,
  "data": { "sessionId": "temp_123", "processId": 4321 }
}`,
  },
  {
    id: "temporary.customLaunch",
    groupId: "temporary",
    titleKey: "api.endpoint.temporary.customLaunch",
    descriptionKey: "api.endpoint.temporary.customLaunch.description",
    method: "POST",
    path: "/v1/temp-profiles/custom-launch",
    protection: "standard",
    bodyExample: `{
  "name": "One-time browser",
  "os": "windows",
  "proxyId": null,
  "fingerprint": {}
}`,
    responseExample: `{
  "success": true,
  "data": { "sessionId": "temp_456", "processId": 4322 }
}`,
  },
  {
    id: "temporary.close",
    groupId: "temporary",
    titleKey: "api.endpoint.temporary.close",
    descriptionKey: "api.endpoint.temporary.close.description",
    method: "POST",
    path: "/v1/temp-profiles/{sessionId}/close",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": { "closed": true, "dataDeleted": true }
}`,
  },
  {
    id: "cookies.get",
    groupId: "cookies",
    titleKey: "api.endpoint.cookies.get",
    descriptionKey: "api.endpoint.cookies.get.description",
    method: "GET",
    path: "/v1/profiles/{profileId}/cookies",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": [{ "name": "sid", "domain": ".example.com", "value": "..." }]
}`,
  },
  {
    id: "cookies.update",
    groupId: "cookies",
    titleKey: "api.endpoint.cookies.update",
    descriptionKey: "api.endpoint.cookies.update.description",
    method: "PUT",
    path: "/v1/profiles/{profileId}/cookies",
    protection: "sensitive",
    bodyExample: `{
  "cookies": [{ "name": "sid", "domain": ".example.com", "value": "..." }]
}`,
    responseExample: `{
  "success": true,
  "data": { "updated": 1 }
}`,
  },
  {
    id: "fingerprints.update",
    groupId: "fingerprints",
    titleKey: "api.endpoint.fingerprints.update",
    descriptionKey: "api.endpoint.fingerprints.update.description",
    method: "PATCH",
    path: "/v1/profiles/{profileId}/fingerprint",
    protection: "standard",
    bodyExample: `{
  "fingerprint": { "locale": "en-US", "timezone": "America/New_York" }
}`,
    responseExample: `{
  "success": true,
  "data": { "id": "profile_123", "fingerprint": {} }
}`,
  },
  {
    id: "groups.list",
    groupId: "groups",
    titleKey: "api.endpoint.groups.list",
    descriptionKey: "api.endpoint.groups.list.description",
    method: "GET",
    path: "/v1/groups",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": [{ "id": "group_123", "name": "Default" }]
}`,
  },
  {
    id: "groups.create",
    groupId: "groups",
    titleKey: "api.endpoint.groups.create",
    descriptionKey: "api.endpoint.groups.create.description",
    method: "POST",
    path: "/v1/groups",
    protection: "standard",
    bodyExample: `{ "name": "Storefront", "color": "#5640c9" }`,
    responseExample: `{
  "success": true,
  "data": { "id": "group_123", "name": "Storefront" }
}`,
  },
  {
    id: "groups.update",
    groupId: "groups",
    titleKey: "api.endpoint.groups.update",
    descriptionKey: "api.endpoint.groups.update.description",
    method: "PATCH",
    path: "/v1/groups/{groupId}",
    protection: "standard",
    bodyExample: `{ "name": "Storefront Updated" }`,
    responseExample: `{
  "success": true,
  "data": { "id": "group_123", "name": "Storefront Updated" }
}`,
  },
  {
    id: "groups.delete",
    groupId: "groups",
    titleKey: "api.endpoint.groups.delete",
    descriptionKey: "api.endpoint.groups.delete.description",
    method: "DELETE",
    path: "/v1/groups/{groupId}",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": { "deleted": true }
}`,
  },
  {
    id: "extensions.list",
    groupId: "extensions",
    titleKey: "api.endpoint.extensions.list",
    descriptionKey: "api.endpoint.extensions.list.description",
    method: "GET",
    path: "/v1/extensions",
    protection: "standard",
    responseExample: `{
  "success": true,
  "data": [{ "id": "extension_123", "name": "Local extension" }]
}`,
  },
  {
    id: "extensions.add",
    groupId: "extensions",
    titleKey: "api.endpoint.extensions.add",
    descriptionKey: "api.endpoint.extensions.add.description",
    method: "POST",
    path: "/v1/extensions",
    protection: "standard",
    bodyExample: `{ "localPath": "D:\\\\Downloads\\\\extension.zip" }`,
    responseExample: `{
  "success": true,
  "data": { "id": "extension_123", "name": "Local extension" }
}`,
  },
  {
    id: "extensions.delete",
    groupId: "extensions",
    titleKey: "api.endpoint.extensions.delete",
    descriptionKey: "api.endpoint.extensions.delete.description",
    method: "DELETE",
    path: "/v1/extensions/{extensionId}",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": { "deleted": true }
}`,
  },
  {
    id: "cache.clear",
    groupId: "cache",
    titleKey: "api.endpoint.cache.clear",
    descriptionKey: "api.endpoint.cache.clear.description",
    method: "DELETE",
    path: "/v1/profiles/{profileId}/cache",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": { "cleared": true }
}`,
  },
  {
    id: "profiles.deleteData",
    groupId: "cache",
    titleKey: "api.endpoint.profiles.deleteData",
    descriptionKey: "api.endpoint.profiles.deleteData.description",
    method: "DELETE",
    path: "/v1/profiles/{profileId}/data",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": { "deleted": true }
}`,
  },
  {
    id: "credentials.list",
    groupId: "credentials",
    titleKey: "api.endpoint.credentials.list",
    descriptionKey: "api.endpoint.credentials.list.description",
    method: "GET",
    path: "/v1/profiles/{profileId}/credentials",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": [{ "id": "credential_123", "platform": "example.com", "username": "user" }]
}`,
  },
  {
    id: "credentials.create",
    groupId: "credentials",
    titleKey: "api.endpoint.credentials.create",
    descriptionKey: "api.endpoint.credentials.create.description",
    method: "POST",
    path: "/v1/profiles/{profileId}/credentials",
    protection: "sensitive",
    bodyExample: `{
  "platform": "example.com",
  "username": "user",
  "password": "secret"
}`,
    responseExample: `{
  "success": true,
  "data": { "id": "credential_123" }
}`,
  },
  {
    id: "credentials.update",
    groupId: "credentials",
    titleKey: "api.endpoint.credentials.update",
    descriptionKey: "api.endpoint.credentials.update.description",
    method: "PATCH",
    path: "/v1/profiles/{profileId}/credentials/{credentialId}",
    protection: "sensitive",
    bodyExample: `{ "username": "updated-user", "password": "new-secret" }`,
    responseExample: `{
  "success": true,
  "data": { "id": "credential_123", "updated": true }
}`,
  },
  {
    id: "credentials.delete",
    groupId: "credentials",
    titleKey: "api.endpoint.credentials.delete",
    descriptionKey: "api.endpoint.credentials.delete.description",
    method: "DELETE",
    path: "/v1/profiles/{profileId}/credentials/{credentialId}",
    protection: "sensitive",
    responseExample: `{
  "success": true,
  "data": { "deleted": true }
}`,
  },
];

export function getEndpointById(id: string): ApiEndpoint {
  return apiEndpoints.find((endpoint) => endpoint.id === id) ?? apiEndpoints[0]!;
}

export function endpointsByGroup(groupId: ApiGroupId): ApiEndpoint[] {
  return apiEndpoints.filter((endpoint) => endpoint.groupId === groupId);
}

export function buildSampleRequest(endpoint: ApiEndpoint, baseUrl: string, apiKeySuffix: string) {
  const url = `${baseUrl}${endpoint.path.replace(/\{[^}]+\}/g, "sample-id")}`;
  return `fetch("${url}", {
  method: "${endpoint.method}",
  headers: {
    "Authorization": "Bearer <selected-api-key:${apiKeySuffix || "none"}>",
    "Content-Type": "application/json"
  }${endpoint.bodyExample ? `,\n  body: JSON.stringify(${endpoint.bodyExample})` : ""}
})
  .then((response) => response.json())
  .then((data) => console.log(data));`;
}
