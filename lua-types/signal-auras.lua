---@meta

---@class SignalAurasMacroAction

---@alias SignalAurasMacro SignalAurasMacroAction[]

---Creates an ordered Signal Auras macro definition.
---@param actions SignalAurasMacroAction[]
---@return SignalAurasMacro
function macro(actions) end

---Creates a key press macro action.
---@param name string
---@return SignalAurasMacroAction
function key(name) end

---Creates a text input macro action.
---@param value string
---@return SignalAurasMacroAction
function text(value) end

---Creates a delay macro action in milliseconds.
---@param ms integer
---@return SignalAurasMacroAction
function delay(ms) end
