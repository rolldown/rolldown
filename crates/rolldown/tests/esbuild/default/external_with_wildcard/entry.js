// Should match
import "/assets/images/test.jpg";
import "/dir/x/file.gif";
import "/dir//file.gif";
import "./file.png";

// Should not match
import "/sassets/images/test.jpg";
import "/dir/file.gif";
import "./file.ping";