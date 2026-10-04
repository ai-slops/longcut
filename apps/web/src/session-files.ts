export type PackageFile = {
  readonly file: File
  readonly path: string
}

export class PackageSelectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "PackageSelectionError"
  }
}

export function preparePackageFiles(files: readonly File[]): readonly PackageFile[] {
  if (files.length === 0) throw new PackageSelectionError("Select a completed package directory.")

  const roots = new Set<string>()
  const entries = files.map((file): PackageFile => {
    const parts = file.webkitRelativePath.split("/").filter((part) => part.length > 0)
    if (parts.length > 1) {
      const root = parts[0]
      if (root) roots.add(root)
      return { file, path: parts.slice(1).join("/") }
    }
    return { file, path: file.name }
  })

  if (roots.size > 1) {
    throw new PackageSelectionError("Select one package directory at a time.")
  }
  if (!entries.some(({ path }) => path === "manifest.json")) {
    throw new PackageSelectionError(
      "The selected directory does not contain manifest.json at its root.",
    )
  }
  return entries
}
