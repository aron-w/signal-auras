input_provider = {
  backend = "evdev",
  mode = "grab",
  output = "uinput",
  devices = {
    "/dev/input/by-signal-auras/keychron-k5-pro",
    "/dev/input/by-signal-auras/logitech-mouse",
    "/dev/input/by-signal-auras/logitech-mouse-keyboard",
  },
}

leader = "F9"

sa.press({
  trigger = "F5",
  -- scope = { processes = { "steam_app_2694490" } },
  mode = "passthrough",
  callback = "go_home",
})

sa.motion({
  requires_held = { "<Leader>" },
  trigger = "<LClick> <LClick>",
  mode = "passthrough",
  callback = "ctrl_down",
  loop = {
    while_held = { "<LClick>" },
    before = "ctrl_down",
    repeat = {
      every_ms = 65,
      callback = "click_left",
    },
    after = "ctrl_up",
  },
})

sa.press({
  requires_held = { "<Leader>" },
  trigger = "<WheelUp>",
  mode = "passthrough",
  callback = "previous_panel",
})

sa.press({
  requires_held = { "<Leader>" },
  trigger = "<WheelDown>",
  mode = "passthrough",
  callback = "next_panel",
})

sa.press({
  requires_held = { "<Leader>" },
  trigger = "<RClick>",
  mode = "consume",
  callback = "alt_click",
})

sa.press({
  requires_held = { "<Leader>" },
  trigger = "<LClick>",
  mode = "consume",
  callback = "ctrl_click",
})

sa.callback("go_home", function()
  sa.input.key("Enter")
  sa.input.text("/hideout")
  sa.input.key("Enter")
end)

sa.callback("previous_panel", function()
  sa.input.key("Left")
end)

sa.callback("next_panel", function()
  sa.input.key("Right")
end)

sa.callback("alt_click", function()
  sa.input.key_down("Alt")
  sa.input.mouse_click("left")
  sa.input.key_up("Alt")
end)

sa.callback("ctrl_down", function()
  sa.input.key_down("Ctrl")
end)

sa.callback("click_left", function()
  sa.input.mouse_click("left")
end)

sa.callback("ctrl_up", function()
  sa.input.key_up("Ctrl")
end)

sa.callback("ctrl_click", function()
  sa.input.key_down("Ctrl")
  sa.input.mouse_click("left")
  sa.input.key_up("Ctrl")
end)
