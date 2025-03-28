import {
  countModuleGraphSizeRs as count_module_graph_size_rs,
  isBarrelFileRs as is_barrel_file,
  resolveRs as resolve_rs
} from './rs.cjs';
import { builtinModules } from "module";

/**
 * @param {string} importer - the file that is importing the module
 * @param {string} importee - the module being imported
 * @param {{
 *  mainFields?: string[],
 *  exportConditions?: string[],
*  extensions?: string[],
* }} [options]
* @returns {string} the resolved path to the module
*/
export function resolve(importer, importee, options) {
  const mainFields = options?.mainFields || ["module", "browser", "main"];
  const exportConditions = options?.exportConditions || ["node", "import"];
  const extensions = options?.extensions || [".js", ".ts", ".tsx", ".jsx", ".json", ".node"];
  const tsConfigFile = options?.tsconfig?.configFile;
  const tsReferences = options?.tsconfig?.references;

  return resolve_rs(importer, importee, exportConditions, mainFields, extensions, tsConfigFile, tsReferences);
}

/**
* @param {string[]} entrypoints 
* @param {{
*  basePath?: string,
*  exportConditions?: string[],
*  mainFields?: string[],
*  extensions?: string[],
* }} [options]
* @returns {number}
*/
export function count_module_graph_size(entrypoints, options = {}) {
  const {
    basePath = process.cwd(),
    exportConditions = ["node", "import"],
    mainFields = ["module", "browser", "main"],
    extensions = [".js", ".ts", ".tsx", ".jsx", ".json", ".node"],
    ignoreModuleExtensions,
    tsconfig,
  } = options;

  const tsConfigFile = tsconfig?.configFile;
  const tsReferences = tsconfig?.references;

  // alias is expected to be a vector
  let alias = options.alias ?? [];
  if (options.alias && typeof options.alias === 'object') {
    alias = Object.entries(options.alias);
  }

  const processedEntrypoints = (typeof entrypoints === "string" ? [entrypoints] : entrypoints);
  const result = count_module_graph_size_rs(
    processedEntrypoints,
    basePath,
    exportConditions,
    mainFields,
    extensions,
    ignoreModuleExtensions,
    builtinModules,
    tsConfigFile,
    tsReferences,
    alias
  );
  return result;
}

export { is_barrel_file };
