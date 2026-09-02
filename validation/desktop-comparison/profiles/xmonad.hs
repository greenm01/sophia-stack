-- Repository-owned xmonad 0.18.1 / xmonad-contrib 0.18.2 comparison profile.
import XMonad
import XMonad.Hooks.EwmhDesktops (ewmh, ewmhFullscreen)
import XMonad.Hooks.ManageDocks (docks)

main :: IO ()
main =
  xmonad
    . ewmhFullscreen
    . ewmh
    . docks
    $ def
      { borderWidth = 0
      , modMask = mod4Mask
      , terminal = "kitty"
      , manageHook = className =? "sophia-desktop-comparison" --> doFloat
      , layoutHook = Full
      , startupHook =
          spawn
            "xrandr --output DP-1 --mode 2560x1440 --rate 60 --pos 0x0 --primary --output DP-2 --mode 1920x1080 --rate 60 --pos 2560x0"
      }
