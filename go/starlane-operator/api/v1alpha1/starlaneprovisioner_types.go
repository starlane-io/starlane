/*
Copyright 2021.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package v1alpha1

import (
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// EDIT THIS FILE!  THIS IS SCAFFOLDING FOR YOU TO OWN!
// NOTE: json tags are required.  Any new fields you add must have json tags for the fields to be serialized.

// StarlaneProvisionerSpec defines the desired state of StarlaneProvisioner
type StarlaneProvisionerSpec struct {
	// INSERT ADDITIONAL SPEC FIELDS - desired state of cluster
	// Important: Run "make" to regenerate code after modifying this file

	InitArgsArtifact string `json:"init-args-artifact,omitempty"`
	Kind             string `json:"kind"`
	Image            string `json:"image"`

	// List of environment variables to set in the container.
	// Cannot be updated.
	// +optional
	// +patchMergeKey=name
	// +patchStrategy=merge
	Env []corev1.EnvVar `json:"env,omitempty" patchStrategy:"merge" patchMergeKey:"name" protobuf:"bytes,7,rep,name=env"`
}

// StarlaneProvisionerStatus defines the observed state of StarlaneProvisioner
type StarlaneProvisionerStatus struct {
	// INSERT ADDITIONAL STATUS FIELD - define observed state of cluster
	// Important: Run "make" to regenerate code after modifying this file
}

//+kubebuilder:object:root=true
//+kubebuilder:subresource:status

// StarlaneProvisioner is the Schema for the starlaneprovisioners API
type StarlaneProvisioner struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   StarlaneProvisionerSpec   `json:"spec,omitempty"`
	Status StarlaneProvisionerStatus `json:"status,omitempty"`
}

//+kubebuilder:object:root=true

// StarlaneProvisionerList contains a list of StarlaneProvisioner
type StarlaneProvisionerList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []StarlaneProvisioner `json:"items"`
}

func init() {
	SchemeBuilder.Register(&StarlaneProvisioner{}, &StarlaneProvisionerList{})
}
