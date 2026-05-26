---@meta

---@class SignalAurasMacroAction

---@alias SignalAurasMacro SignalAurasMacroAction[]
---@alias SignalAurasModifier '"Ctrl"'|'"Alt"'|'"Shift"'|'"Super"'
---@alias SignalAurasBindingMode '"consume"'|'"passthrough"'
---@alias SignalAurasMouseButton '"left"'|'"right"'|'"middle"'
---@alias SignalAurasWheelDirection '"up"'|'"down"'

---@class SignalAurasMouseTrigger
---@field button? SignalAurasMouseButton
---@field wheel? SignalAurasWheelDirection

---@class SignalAurasBindingTrigger
---@field modifiers? SignalAurasModifier[]
---@field mouse? SignalAurasMouseTrigger
---@field key? string

---@class SignalAurasBinding
---@field trigger SignalAurasBindingTrigger
---@field mode? SignalAurasBindingMode
---@field macro SignalAurasMacro

---@class SignalAurasConfig
---@field hotkeys? table<string, SignalAurasMacro>
---@field bindings? SignalAurasBinding[]

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
