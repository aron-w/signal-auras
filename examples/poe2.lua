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

poe = { processes = { "steam_app_2694490", "PathOfExileSteam.exe" } }

leader = "F9"

sa.state.track({
  id = "refutation_cooldown",
  scope = poe,
  capabilities = { "screen_read" },
  poll_ms = 50,
  detector = {
    kind = "radial_cooldown",
    roi = { x = 1923, y = 1370, w = 36, h = 36 },
    mask = { shape = "circle", inset = 10 },
    prediction = {
      duration_ms = 7000,
      stable_after_ms = 500,
    },
    phases = {
      order = { "ready", "activated", "active", "recovering" },
      fallback = "unknown",

      ready = {
        sample = {
          kind = "clock_probe",
          angle_deg = 352,
          radius_px = 15,
          w = 3,
          h = 3,
        },
        min_luminance_percent = 44,
        min_saturation = 85,
        progress_fill = "full",
      },

      activated = {
        sample = {
          kind = "clock_probe",
          angle_deg = 8,
          radius_px = 15,
          w = 3,
          h = 3,
        },
        max_luminance_percent = 12,
        max_saturation = 20,
        progress_fill = "empty",
      },

      active = {
        sample = {
          kind = "clock_probe",
          angle_deg = 8,
          radius_px = 15,
          w = 3,
          h = 3,
        },
        max_luminance_percent = 34,
        max_saturation = 75,
        progress_fill = "empty",
      },

      recovering = {
        sample = {
          kind = "annulus_arc",
          inner_radius_px = 13,
          outer_radius_px = 17,
          start_deg = 20,
          end_deg = 340,
        },
        min_luminance_percent = 40,
        min_saturation = 80,
        metric = "bright_ratio",
        metric_scale = 1.5,
        progress_fill = "fraction",
        max_fill_until_ready = 0.95,
      },
    },
  },
})

sa.state.track({
  id = "heavy_stun",
  scope = poe,
  capabilities = { "screen_read" },
  poll_ms = 50,
  when = { tracker = "refutation_cooldown", phase = "active" },
  detector = {
    kind = "horizontal_progress_bar",
    roi = { x = 315, y = 1250, w = 300, h = 2 },
    fill = { direction = "left_to_right" },
  },
})

sa.overlay.mount({
  id = "poe2_status",
  scope = poe,
  provider = "native",
  surface = "overlay",
  hotkey = { trigger = "Shift+F1", mode = "consume" },
  visuals = {
    {
      id = "heavy_stun",
      kind = "progress_bar",
      bind = { tracker = "heavy_stun", field = "progress_percent" },
      rect = { x = 1200, y = 900, w = 150, h = 22 },
      opacity = 0.72,
      fill = "#d8b84c",
      background = "#101820",
      label = { visible = true },
      inactive = { opacity = 0.0 },
    },
    {
      id = "refutation",
      kind = "progress_bar",
      bind = { tracker = "refutation_cooldown", field = "remaining_ms" },
      rect = { x = 1200, y = 930, w = 150, h = 22 },
      opacity = 0.72,
      fill = "#5aa7ff",
      background = "#101820",
      label = { visible = true },
      ready = { fill = "#4ade80", opacity = 0.85 },
      activated = { fill = "#f97316", background = "#7f1d1d", opacity = 0.85 },
      active = { fill = "#38bdf8", background = "#082f49", opacity = 0.8 },
      inactive = { opacity = 0.25 },
    },
  },
})

sa.press({
  trigger = "F5",
  -- scope = { processes = { "steam_app_2694490" } },
  mode = "passthrough",
  callback = "go_home",
})

sa.press({
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

sa.motion({
  requires_held = { "<Leader>" },
  trigger = "<LClick> <LClick>",
  mode = "passthrough",
  callback = "ctrl_down",
  loop = {
    while_held = { "<LClick>" },
    before = "ctrl_down",
    repeat = {
      every_ms = 40,
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

sa.callback("reload_filterblade", function()
  sa.sleep(100)

  local active = sa.window.active({ title = true })
  sa.log("filterblade active_title=" .. tostring(active.title))
  local filter = active.title and active.title:match("^FilterBlade%s+%-%s+(.-)%s+%-%s+FilterBlade")
  if filter == nil then
    filter = active.title and active.title:match("^(.-)%s+%-%s+FilterBlade")
  end
  sa.log("filterblade parsed_filter=" .. tostring(filter))
  if filter == nil or filter == "" then
    sa.log_warn("filterblade no matching FilterBlade title")
    return
  end

  sa.log("filterblade finding_poe processes=steam_app_2694490,PathOfExileSteam.exe")
  local poe = sa.window.find({
    processes = { "steam_app_2694490", "PathOfExileSteam.exe" },
  })
  sa.log("filterblade poe_handle=" .. tostring(poe))
  if poe == nil then
    sa.log_warn("filterblade poe window not found")
    return
  end

  local activated = sa.window.activate(poe)
  sa.log("filterblade activated=" .. tostring(activated))
  if not activated then
    return
  end
  local focused = sa.window.wait_active(poe, 500)
  sa.log("filterblade focused=" .. tostring(focused))
  if not focused then
    return
  end

  sa.sleep(150)
  local command = "/reloaditemfilter " .. filter
  sa.log("filterblade command=" .. command)
  sa.input.key("Enter")
  sa.input.text(command)
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
