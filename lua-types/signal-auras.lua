---@meta

---@class SignalAurasMacroAction

---@alias SignalAurasMacro SignalAurasMacroAction[]
---@alias SignalAurasModifier '"Ctrl"'|'"Alt"'|'"Shift"'|'"Super"'
---@alias SignalAurasBindingMode '"consume"'|'"passthrough"'
---@alias SignalAurasMouseButton '"left"'|'"right"'|'"middle"'
---@alias SignalAurasWheelDirection '"up"'|'"down"'
---@alias SignalAurasKeyName string Key names are normalized by Signal Auras; common aliases include Enter, Return, Esc, Escape, F1-F24, PageUp, PageDown, KPEnter, VolumeUp, and one-character keys.
---@alias SignalAurasMotionToken '"<Leader>"'|'"<LClick>"'|'"<RClick>"'|'"<MClick>"'|'"<WheelUp>"'|'"<WheelDown>"'|string
---@alias SignalAurasHeldToken '"<Leader>"'|'"<LClick>"'|'"<RClick>"'|'"<MClick>"'|string
---@alias SignalAurasInputProviderBackend '"evdev"'
---@alias SignalAurasInputProviderMode '"observe"'|'"grab"'|'"consume"'
---@alias SignalAurasInputProviderOutput '"portal"'|'"uinput"'

---@class SignalAurasMouseTrigger
---@field button? SignalAurasMouseButton
---@field wheel? SignalAurasWheelDirection

---@class SignalAurasBindingTrigger
---@field modifiers? SignalAurasModifier[]
---@field mouse? SignalAurasMouseTrigger
---@field key? SignalAurasKeyName

---@class SignalAurasBinding
---@field trigger SignalAurasBindingTrigger
---@field mode? SignalAurasBindingMode
---@field macro SignalAurasMacro

---@class SignalAurasDefaults
---@field inter_action_delay_ms? integer

---@class SignalAurasLoopRepeat
---@field every_ms integer
---@field macro SignalAurasMacro

---@class SignalAurasLoop
---@field while_held SignalAurasMotionToken[]
---@field before? SignalAurasMacro
---@field once? SignalAurasMacro
---@field repeat? SignalAurasLoopRepeat
---@field after? SignalAurasMacro

---@class SignalAurasMotion
---@field requires_held? SignalAurasHeldToken[]
---@field trigger SignalAurasMotionToken[]
---@field within_ms? integer
---@field mode? SignalAurasBindingMode
---@field macro? SignalAurasMacro
---@field loop? SignalAurasLoop
---@field inter_action_delay_ms? integer

---@class SignalAurasPress
---@field requires_held? SignalAurasHeldToken[]
---@field trigger SignalAurasMotionToken
---@field mode? SignalAurasBindingMode
---@field macro SignalAurasMacro
---@field inter_action_delay_ms? integer

---@class SignalAurasInputProvider
---@field backend SignalAurasInputProviderBackend
---@field mode? SignalAurasInputProviderMode
---@field output? SignalAurasInputProviderOutput
---@field devices string[]|'"all"'
---@field acknowledge_risk? string

---@class SignalAurasConfig
---@field leader? SignalAurasKeyName
---@field defaults? SignalAurasDefaults
---@field input_provider? SignalAurasInputProvider
---@field hotkeys? table<string, SignalAurasMacro>
---@field bindings? SignalAurasBinding[]
---@field motions? SignalAurasMotion[]
---@field presses? SignalAurasPress[]

---Creates an ordered Signal Auras macro definition.
---@param actions SignalAurasMacroAction[]
---@return SignalAurasMacro
function macro(actions) end

---Creates a key press macro action.
---@param name SignalAurasKeyName
---@return SignalAurasMacroAction
function key(name) end

---Creates a key press-only macro action.
---@param name SignalAurasKeyName
---@return SignalAurasMacroAction
function key_down(name) end

---Creates a key release-only macro action.
---@param name SignalAurasKeyName
---@return SignalAurasMacroAction
function key_up(name) end

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
