---@param aura SignalAurasController
local function configure(aura)
aura.configure({
  input_provider = {
    backend = "evdev",
    mode = "grab",
    output = "uinput",
    devices = "interactive",
  },
  leader = "F9",
})

local poe = { processes = { "steam_app_2694490", "PathOfExileSteam.exe" } }
aura.press({
  trigger = "F5",
  -- scope = { processes = { "steam_app_2694490" } },
  mode = "passthrough",
  callback = "go_home",
})

aura.press({
  requires_held = { "Ctrl" },
  trigger = "S",
  mode = "passthrough",
  capabilities = {
    "active_window_metadata",
    "window_activation",
    "synthesized_input",
    "timer",
  },
  callback = "reload_filterblade",
})

aura.motion({
  requires_held = { "<Leader>" },
  trigger = "<LClick> <LClick>",
  mode = "passthrough",
  callback = "ctrl_down",
  loop = {
    while_held = { "<LClick>" },
    before = "ctrl_down",
    ["repeat"] = {
      every_ms = 40,
      callback = "click_left",
    },
    after = "ctrl_up",
  },
})

aura.press({
  requires_held = { "<Leader>" },
  trigger = "<WheelUp>",
  mode = "passthrough",
  callback = "previous_panel",
})

aura.press({
  requires_held = { "<Leader>" },
  trigger = "<WheelDown>",
  mode = "passthrough",
  callback = "next_panel",
})

aura.press({
  requires_held = { "<Leader>" },
  trigger = "<RClick>",
  mode = "consume",
  callback = "alt_click",
})

aura.press({
  requires_held = { "<Leader>" },
  trigger = "<LClick>",
  mode = "consume",
  callback = "ctrl_click",
})

aura.callback("go_home", function()
  aura.input.key("Enter")
  aura.input.text("/hideout")
  aura.input.key("Enter")
end)

aura.callback("reload_filterblade", function()
  aura.sleep(100)

  local active = aura.window.active({ title = true })
  aura.log("filterblade active_title=" .. tostring(active.title))
  local filter = active.title and active.title:match("^FilterBlade%s+%-%s+(.-)%s+%-%s+FilterBlade")
  if filter == nil then
    filter = active.title and active.title:match("^(.-)%s+%-%s+FilterBlade")
  end
  aura.log("filterblade parsed_filter=" .. tostring(filter))
  if filter == nil or filter == "" then
    aura.log_warn("filterblade no matching FilterBlade title")
    return
  end

  aura.log("filterblade finding_poe processes=steam_app_2694490,PathOfExileSteam.exe")
  local poe_window = aura.window.find({
    processes = { "steam_app_2694490", "PathOfExileSteam.exe" },
  })
  aura.log("filterblade poe_handle=" .. tostring(poe_window))
  if poe_window == nil then
    aura.log_warn("filterblade poe window not found")
    return
  end

  local activated = aura.window.activate(poe_window)
  aura.log("filterblade activated=" .. tostring(activated))
  if not activated then
    return
  end
  local focused = aura.window.wait_active(poe_window, 500)
  aura.log("filterblade focused=" .. tostring(focused))
  if not focused then
    return
  end

  aura.sleep(150)
  local command = "/reloaditemfilter " .. filter
  aura.log("filterblade command=" .. command)
  aura.input.key("Enter")
  aura.input.text(command)
  aura.input.key("Enter")
end)

aura.callback("previous_panel", function()
  aura.input.key("Left")
end)

aura.callback("next_panel", function()
  aura.input.key("Right")
end)

aura.callback("alt_click", function()
  aura.input.key_down("Alt")
  aura.input.mouse_click("left")
  aura.input.key_up("Alt")
end)

aura.callback("ctrl_down", function()
  aura.input.key_down("Ctrl")
end)

aura.callback("click_left", function()
  aura.input.mouse_click("left")
end)

aura.callback("ctrl_up", function()
  aura.input.key_up("Ctrl")
end)

aura.callback("ctrl_click", function()
  aura.input.key_down("Ctrl")
  aura.input.mouse_click("left")
  aura.input.key_up("Ctrl")
end)

end

return configure
