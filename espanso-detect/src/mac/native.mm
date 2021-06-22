/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

#include "native.h"
#import <AppKit/AppKit.h>
#import <Foundation/Foundation.h>
#include <Carbon/Carbon.h>

#include <string.h>

const unsigned long long FLAGS = NSEventMaskKeyDown | NSEventMaskKeyUp | NSEventMaskFlagsChanged | NSEventMaskLeftMouseDown | 
                                 NSEventMaskLeftMouseUp | NSEventMaskRightMouseDown | NSEventMaskRightMouseUp | 
                                 NSEventMaskOtherMouseDown | NSEventMaskOtherMouseUp;

OSStatus hotkey_event_handler(EventHandlerCallRef _next, EventRef evt, void *userData);

void * detect_initialize(EventCallback callback, InitializeOptions options) {
  HotKey * hotkeys_clone = (HotKey*) malloc(sizeof(HotKey) * options.hotkeys_count);
  memcpy(hotkeys_clone, options.hotkeys, sizeof(HotKey) * options.hotkeys_count);

  dispatch_async(dispatch_get_main_queue(), ^(void) {
    // Setup hotkeys
    if (options.hotkeys_count > 0) {
      EventHotKeyRef hotkey_ref;
      EventHotKeyID hotkey_id;
      hotkey_id.signature='htk1';

      EventTypeSpec eventType;
      eventType.eventClass = kEventClassKeyboard;
      eventType.eventKind = kEventHotKeyPressed;    
      
      InstallApplicationEventHandler(&hotkey_event_handler, 1, &eventType, (void*)callback, NULL);

      for (int i = 0; i<options.hotkeys_count; i++) {
        hotkey_id.id=hotkeys_clone[i].hk_id;
        RegisterEventHotKey(hotkeys_clone[i].key_code, hotkeys_clone[i].flags, hotkey_id, GetApplicationEventTarget(), 0, &hotkey_ref);  
      }
    }
    
    free(hotkeys_clone);

    // Setup key detection

    [NSEvent addGlobalMonitorForEventsMatchingMask:FLAGS handler:^(NSEvent *event){
        InputEvent inputEvent = {};
        if (event.type == NSEventTypeKeyDown || event.type == NSEventTypeKeyUp ) {
          inputEvent.event_type = INPUT_EVENT_TYPE_KEYBOARD;
          inputEvent.status = (event.type == NSEventTypeKeyDown) ? INPUT_STATUS_PRESSED : INPUT_STATUS_RELEASED;
          inputEvent.key_code = event.keyCode;

          const char *chars = [event.characters UTF8String];
          strncpy(inputEvent.buffer, chars, 23);
          inputEvent.buffer_len = event.characters.length;

          callback(inputEvent);
        }else if (event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeRightMouseDown || event.type == NSEventTypeOtherMouseDown ||
                  event.type == NSEventTypeLeftMouseUp || event.type == NSEventTypeRightMouseUp || event.type == NSEventTypeOtherMouseUp) {
          inputEvent.event_type = INPUT_EVENT_TYPE_MOUSE;
          inputEvent.status = (event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeRightMouseDown ||
                               event.type == NSEventTypeOtherMouseDown) ? INPUT_STATUS_PRESSED : INPUT_STATUS_RELEASED;
          if (event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeLeftMouseUp) {
            inputEvent.key_code = INPUT_MOUSE_LEFT_BUTTON;
          } else if (event.type == NSEventTypeRightMouseDown || event.type == NSEventTypeRightMouseUp) {
            inputEvent.key_code = INPUT_MOUSE_RIGHT_BUTTON;
          } else if (event.type == NSEventTypeOtherMouseDown || event.type == NSEventTypeOtherMouseUp) {
            inputEvent.key_code = INPUT_MOUSE_MIDDLE_BUTTON;
          }

          callback(inputEvent);
        }else{
          // Modifier keys (SHIFT, CTRL, ecc) are handled as a separate case on macOS
          inputEvent.event_type = INPUT_EVENT_TYPE_KEYBOARD;
          inputEvent.key_code = event.keyCode;

          // To determine whether these keys are pressed or released, we have to analyze each case
          if (event.keyCode == kVK_Shift || event.keyCode == kVK_RightShift) {
            inputEvent.status = (([event modifierFlags] & NSEventModifierFlagShift) == 0) ? INPUT_STATUS_RELEASED : INPUT_STATUS_PRESSED;
          } else if (event.keyCode == kVK_Command || event.keyCode == kVK_RightCommand) {
            inputEvent.status = (([event modifierFlags] & NSEventModifierFlagCommand) == 0) ? INPUT_STATUS_RELEASED : INPUT_STATUS_PRESSED;
          } else if (event.keyCode == kVK_Control || event.keyCode == kVK_RightControl) {
            inputEvent.status = (([event modifierFlags] & NSEventModifierFlagControl) == 0) ? INPUT_STATUS_RELEASED : INPUT_STATUS_PRESSED;
          } else if (event.keyCode == kVK_Option || event.keyCode == kVK_RightOption) {
            inputEvent.status = (([event modifierFlags] & NSEventModifierFlagOption) == 0) ? INPUT_STATUS_RELEASED : INPUT_STATUS_PRESSED;
          }
          callback(inputEvent);
        }
    }];
  });
}

OSStatus hotkey_event_handler(EventHandlerCallRef _next, EventRef evt, void *userData)
{
    EventHotKeyID hotkey_id;
    GetEventParameter(evt, kEventParamDirectObject, typeEventHotKeyID, NULL, sizeof(hotkey_id), NULL, &hotkey_id);
    
    EventCallback callback = (EventCallback) userData;

    InputEvent inputEvent = {};
    inputEvent.event_type = INPUT_EVENT_TYPE_HOTKEY;
    inputEvent.key_code = hotkey_id.id;
    callback(inputEvent);

    return noErr;
}