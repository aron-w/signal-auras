---@meta

---@class SignalAurasMacroAction

---@alias SignalAurasMacro SignalAurasMacroAction[]
---@alias SignalAurasModifier '"Ctrl"'|'"Alt"'|'"Shift"'|'"Super"'
---@alias SignalAurasBindingMode '"consume"'|'"passthrough"'
---@alias SignalAurasMouseButton '"left"'|'"right"'|'"middle"'
---@alias SignalAurasWheelDirection '"up"'|'"down"'
---@alias SignalAurasMotionToken '"<Leader>"'|'"<LClick>"'|'"<RClick>"'|'"<MClick>"'|string
---@alias SignalAurasInputProviderBackend '"evdev"'
---@alias SignalAurasInputProviderMode '"observe"'|'"grab"'|'"consume"'
---@alias SignalAurasInputProviderOutput '"portal"'|'"uinput"'

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

---@class SignalAurasDefaults
---@field inter_action_delay_ms? integer

---@class SignalAurasRepeatInterval
---@field min integer
---@field max integer

---@class SignalAurasRepeat
---@field while_held SignalAurasMotionToken[]
---@field interval_ms SignalAurasRepeatInterval
---@field macro SignalAurasMacro

---@class SignalAurasMotion
---@field trigger SignalAurasMotionToken[]
---@field mode? SignalAurasBindingMode
---@field macro? SignalAurasMacro
---@field repeat? SignalAurasRepeat
---@field inter_action_delay_ms? integer

---@class SignalAurasInputProvider
---@field backend SignalAurasInputProviderBackend
---@field mode? SignalAurasInputProviderMode
---@field output? SignalAurasInputProviderOutput
---@field devices string[]|'"all"'
---@field acknowledge_risk? string

---@class SignalAurasConfig
---@field leader? string
---@field defaults? SignalAurasDefaults
---@field input_provider? SignalAurasInputProvider
---@field hotkeys? table<string, SignalAurasMacro>
---@field bindings? SignalAurasBinding[]
---@field motions? SignalAurasMotion[]

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

---Creates a mouse click macro action.
---@param button SignalAurasMouseButton
---@return SignalAurasMacroAction
function mouse_click(button) end

---Creates a delay macro action in milliseconds.
---@param ms integer
---@return SignalAurasMacroAction
function delay(ms) end
