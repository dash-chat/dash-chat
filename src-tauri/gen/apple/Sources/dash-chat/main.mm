#include "bindings/bindings.h"
#import <Foundation/Foundation.h>

int main(int argc, char * argv[]) {
	// Point Rust's data dir at the App Group container so the
	// PushNotificationsExtension can read the same files.
	NSURL *containerURL = [[NSFileManager defaultManager]
		containerURLForSecurityApplicationGroupIdentifier:@"group.studio.darksoil.dashchat"];
	if (containerURL) {
		setenv("DATA_DIR", containerURL.path.UTF8String, 1);
	}
	ffi::start_app();
	return 0;
}
