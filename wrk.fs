\ Set terminal to raw mode, wait for a key, print message, reset terminal


: wait-key ( -- )
    raw-mode-on
    begin
        key?  \ check if key is pressed
    until
    key drop  \ consume the key
    cr ." Key detected!"
    raw-mode-off ;

\ Usage: type wait-key and press any key to test